# Audit: Browser Agent
URL: http://localhost:1420/browser-agent
Audited at: 2026-04-09T22:31:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
1. `Error: desktop runtime unavailable` at `src/api/backend.ts:17` via `browserGetPolicy` (`src/api/backend.ts:2451`) called from `src/pages/BrowserAgent.tsx:54`
2. `Error: desktop runtime unavailable` at `src/api/backend.ts:17` via `browserSessionCount` (`src/api/backend.ts:2454`) called from `src/pages/BrowserAgent.tsx:55`
3. `Error: desktop runtime unavailable` at `src/api/backend.ts:17` via `listAgents` (`src/api/backend.ts:29`) called from `src/pages/BrowserAgent.tsx:56`
4. (Errors 1-3 repeat once due to React StrictMode double-invoke, 6 total)

### Warnings
none

### Logs
none

### Info
none

### Debug
none

## Overflow

### 1920x1080
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1901 clientWidth=1577 [OVERFLOW +324px]

### 1280x800
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1901 clientWidth=1577 [OVERFLOW +324px]
- NOTE: viewport locked at 1888x951; resize_window cannot shrink below physical display. Measurements identical across all three breakpoints.

### 1024x768
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1901 clientWidth=1577 [OVERFLOW +324px]
- NOTE: identical to 1920x1080 — viewport-invariant.

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| (none) | — | — | — | — |

Main content area contains zero interactive elements. All buttons/inputs are in the sidebar nav or header banner.

**Header banner buttons (outside `<main>`):**
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no type attr) | — | true |
| 2 | Start Jarvis | button (no type attr) | — | true |

## Click sequence

Note: Main content has zero interactive elements. Clicks performed on the two header-banner buttons as they are the only page-level interactive elements outside the sidebar.

### Click 1: "Refresh"
- Pathname before: /browser-agent
- New console: clean (no messages)
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /browser-agent
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /browser-agent
- New console: clean (no messages)
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /browser-agent
- Reverted: n/a

### Skipped (destructive)
(none)

### Total interactive elements found: 0 (in main), 2 (in header banner)
### Elements clicked: 2

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0 (in main)
- Duplicate H1: 2 visible `<h1>` elements both reading "Browser Agent" — one in `header.nexus-shell-header > div.nexus-control-bar`, one in `main > section.holo-panel > div.holo-panel__content`
- ARIA live regions: 0 — status text "0 active sessions" and empty-state "No agents available" are plain `<div>` elements with no `aria-live`, `role="status"`, or `role="alert"`

## Findings

### browser-agent-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: Three distinct `Error: desktop runtime unavailable` thrown on load from `BrowserAgent.tsx:54-56` calling `browserGetPolicy`, `browserSessionCount`, and `listAgents`. All three backend calls fail with unhandled errors (×2 due to StrictMode = 6 total errors).
- IMPACT: Page renders empty-state content but gives user no feedback that data loading failed; errors are swallowed silently.

### browser-agent-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` scrollWidth=2039 vs clientWidth=1888 (+151px horizontal overflow). Clipped by ancestor `overflow:hidden`.
- IMPACT: Background element exceeds viewport; currently masked but would cause horizontal scroll if any ancestor changed overflow policy.

### browser-agent-03
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=1901 clientWidth=1577 (+324px horizontal), scrollHeight=1297 clientHeight=693 (+604px vertical). `overflow:hidden` clips content silently.
- IMPACT: Panel content is clipped by 604px vertically and 324px horizontally; any content that grows beyond the visible area will be invisible and unreachable.

### browser-agent-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements both visible simultaneously — one in `header.nexus-shell-header` (ancestor path: `DIV.flex > DIV.min-w-[280px] > DIV.flex > DIV.nexus-control-bar > HEADER.nexus-shell-header`), one in `main > section.holo-panel > div.holo-panel__content > div`. Both read "Browser Agent".
- IMPACT: Duplicate H1 violates HTML spec (single H1 per page) and confuses screen reader document outline.

### browser-agent-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Status text "0 active sessions" and empty-state message "No agents available. Create an L3+ agent to start browser automation." are rendered as plain `<div>` elements with no `aria-live`, `role="status"`, or `role="alert"` attributes.
- IMPACT: Screen readers will not announce dynamic status changes or empty-state guidance to users.

### browser-agent-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button at (1597, 60) and "Start Jarvis" button at (1705, 60) in header banner produce zero console output, zero network requests, and zero visible changes when clicked in demo mode.
- IMPACT: Buttons are silently inert — no loading spinner, no error toast, no feedback of any kind. User cannot tell whether click was registered.

### browser-agent-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Both "Refresh" and "Start Jarvis" `<button>` elements lack `type` attribute (type is `null`). HTML spec defaults typeless buttons inside forms to `type="submit"`.
- IMPACT: If these buttons are ever placed inside a `<form>`, they will trigger implicit form submission. Best practice: explicit `type="button"`.

### browser-agent-08
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: First `<h1>` "Browser Agent" is inside `header.nexus-shell-header` which is a sibling of `<main>`. The H1 is not inside `<main>` — it is in the control bar header above. The second H1 inside `<main>` is the page-level heading but is nested deep inside `section.holo-panel > div.holo-panel__content > div` rather than being a direct child of main.
- IMPACT: Landmark structure is incorrect — the primary page heading is outside the main content landmark, breaking screen reader navigation expectations.

## Summary
- Gate detected: no
- Total interactive elements: 0 (in main), 2 (in header banner)
- Elements clicked: 2
- P0: 0
- P1: 3
- P2: 5
