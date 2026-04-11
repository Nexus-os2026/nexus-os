# Audit: Governed Control
URL: http://localhost:1420/governed-control
Audited at: 2026-04-09T22:43:51Z
Gate detected: false
Gate type: none

## Console (captured at 1248x671 effective viewport, ALL messages)
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

Note: `mcp__claude-in-chrome__resize_window` does not affect `window.innerWidth`/`innerHeight` in the Tauri-wrapped Chrome DevTools protocol. All three requested viewport sizes resolved to the same effective viewport of 1248x671. Measurements below reflect this single effective viewport.

### 1920x1080 (effective: 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px minor text truncation]

### 1280x800 (effective: 1248x671 — resize had no effect)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing: same as above

### 1024x768 (effective: 1248x671 — resize had no effect)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing: same as above

### holo-panel clipping detail
- `section.holo-panel`: overflow=hidden, scrollWidth=1615 clientWidth=1577, scrollHeight=1451 clientHeight=637
- Content is clipped in both axes due to `overflow: hidden`

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | (empty — no label, no options) | select | n/a | true |
| 2 | Dismiss | button[type=submit] | n/a | true |

### Banner buttons (outside `main`, inside banner region)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 3 | Refresh | button | n/a | true |
| 4 | Start Jarvis | button | n/a | true |

## Click sequence

### Click 1: "Agent select (combobox)"
- Pathname before: /governed-control
- New console: clean
- Network failures: none
- Visible change: none (dropdown opens but has zero options)
- Pathname after: /governed-control
- Reverted: n/a

### Click 2: "Dismiss"
- Pathname before: /governed-control
- New console: clean
- Network failures: none
- Visible change: none — error message "Error: desktop runtime unavailable" persists after click
- Pathname after: /governed-control
- Reverted: n/a

### Click 3: "Refresh" (banner)
- Pathname before: /governed-control
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /governed-control
- Reverted: n/a

### Click 4: "Start Jarvis" (banner)
- Pathname before: /governed-control
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /governed-control
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 2 (main) + 2 (banner) = 4
### Elements clicked: 4 (all)

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `main select` — no aria-label, no associated `<label>`, no id)
- Buttons without accessible name: 0
- Sections without aria-label: 1 (`section.holo-panel.holo-panel-mid.nexus-page-panel`)
- Duplicate H1: 2 — `H1 "Governed Control"` (shell header, outside main) and `H1 "Governed Computer Control"` (inside main)

## Findings

### governed-control-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` scrollWidth=1348 > clientWidth=1248 (+100px). This decorative element extends beyond the viewport boundary.
- IMPACT: Horizontal scrollbar may appear or layout shifts on viewports where body overflow is not clipped.

### governed-control-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` has `overflow: hidden` with scrollWidth=1615 vs clientWidth=1577 (+38px horizontal) and scrollHeight=1451 vs clientHeight=637 (+814px vertical). Content is silently clipped in both axes.
- IMPACT: Page content below the fold is invisible and unreachable by scrolling inside the holo-panel. Users cannot see all governance rules or action history if content grows.

### governed-control-03
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: `main select` (agent selector combobox) has zero `<option>` elements. Clicking opens an empty dropdown.
- IMPACT: User cannot select an agent, rendering the entire governed control page non-functional in demo mode.

### governed-control-04
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `main select` has no `aria-label`, no `aria-labelledby`, no associated `<label>` element, and no `id`. Screen readers announce it as an unlabeled combobox.
- IMPACT: Accessibility violation — users relying on assistive technology cannot identify the purpose of the control.

### governed-control-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Dismiss" button (type=submit) on the error message "Error: desktop runtime unavailable" does nothing when clicked. Error container remains visible. No console output or state change.
- IMPACT: User expects to dismiss the error notification but the button is a silent no-op.

### governed-control-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Error container "Error: desktop runtime unavailable" is a bare `<div>` with no class, no `role="alert"`, and no `aria-live` attribute. Parent element: `<div>` (no class, no role, no aria-live).
- IMPACT: Screen readers will not announce the error automatically. Error notifications should use `role="alert"` or `aria-live="assertive"`.

### governed-control-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two H1 elements on page: `H1 "Governed Control"` in shell header (outside main) and `H1 "Governed Computer Control"` inside main. Pages should have a single H1.
- IMPACT: Confuses screen reader heading navigation and violates heading hierarchy best practices.

### governed-control-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button in banner header produces no console output, no network request, no visible change when clicked in demo mode.
- IMPACT: Button is a silent no-op with no user feedback.

### governed-control-09
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button in banner header produces no console output, no network request, no visible change when clicked in demo mode.
- IMPACT: Button is completely inert with no user feedback indicating why.

### governed-control-10
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons are located in the `[role="banner"]` region, not inside `main.nexus-shell-content`. They are page-specific action buttons placed outside the main content landmark.
- IMPACT: Buttons are semantically disconnected from the page content they control, which can confuse assistive technology users navigating by landmarks.

### governed-control-11
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has no `aria-label` or `aria-labelledby` attribute.
- IMPACT: Screen readers announce a generic unnamed section landmark, reducing navigation utility.

### governed-control-12
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Dismiss" button has `type="submit"` instead of `type="button"`. It is not inside a `<form>` element, but submit-type buttons outside forms can cause unexpected behavior in some contexts.
- IMPACT: Semantic mismatch — a dismiss/close action should be `type="button"`, not `type="submit"`.

## Summary
- Gate detected: no
- Total interactive elements: 4 (2 in main, 2 in banner)
- Elements clicked: 4
- P0: 0
- P1: 4
- P2: 8
