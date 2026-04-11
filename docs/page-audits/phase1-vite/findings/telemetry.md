# Audit: Telemetry
URL: http://localhost:1420/telemetry
Audited at: 2026-04-10T01:02:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1248x671 — viewport locked, see note)
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

> **NOTE:** Viewport is locked at 1248x671 on ultrawide 3440x1440 display. `resize_window` MCP tool has no effect — all three requested viewports (1920x1080, 1280x800, 1024x768) report identical measurements at the actual 1248x671 viewport.

### 1248x671 (actual — all three requested viewports)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]

### 1920x1080
(not measurable — viewport locked at 1248x671)

### 1280x800
(not measurable — viewport locked at 1248x671)

### 1024x768
(not measurable — viewport locked at 1248x671)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | yes |

Note: 2 additional buttons ("Refresh", "Start Jarvis") exist outside `<main>` in the shell header area. These are excluded per audit scope (main content only).

## Click sequence
### Click 1: "Refresh"
- Pathname before: /telemetry
- New console: clean (no messages)
- Network failures: none
- Visible change: none — button is a silent no-op in demo mode
- Pathname after: /telemetry
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 1
### Elements clicked: 1

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### telemetry-01
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements found on page. `document.querySelectorAll('h1').length === 2`. Both contain text "Telemetry" — one in the page header area and one as the section heading in main content.
- IMPACT: Duplicate H1 violates WCAG heading hierarchy; screen readers announce two top-level headings, confusing page structure.

### telemetry-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248 (locked)
- EVIDENCE: `div.living-background` scrollWidth=1348 > clientWidth=1248 (overflow +100px). Background decorative element exceeds viewport width.
- IMPACT: May cause horizontal scrollbar or clipped content on viewports near or below 1248px width.

### telemetry-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248 (locked)
- EVIDENCE: `section.holo-panel.holo-panel-mid` scrollWidth=1019 > clientWidth=937 (overflow +82px). Panel content exceeds its container.
- IMPACT: Content within the holo-panel may be clipped or trigger unwanted horizontal scroll within the section.

### telemetry-04
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: The "Refresh" button inside `<main>` has no `type` attribute (`btn.getAttribute('type') === null`). Additionally, 2 shell-header buttons ("Refresh", "Start Jarvis") also lack `type`.
- IMPACT: Buttons without `type="button"` default to `type="submit"` per HTML spec, which can cause unintended form submission if ever wrapped in a `<form>`.

### telemetry-05
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: The "Refresh" button in main content produces no console output, no network request, and no visible DOM change when clicked. Silent no-op in demo mode.
- IMPACT: User has no feedback that the button was clicked or that it requires backend connectivity. Should show a toast, loading state, or "unavailable in demo" message.

### telemetry-06
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: Main content shows "desktop runtime unavailable" banner. All metric cards display placeholder dashes: HEALTH STATUS "—", VERSION "—", UPTIME "—". AGENTS ACTIVE shows "0". AUDIT CHAIN shows "Invalid". Status Detail and Health Detail show "No ... data available". Telemetry Configuration shows "No configuration available". Total text content in main is only 423 characters.
- IMPACT: Demo mode presents minimal placeholder data with no simulated values, making the page appear broken rather than demonstrating its intended functionality.

## Summary
- Gate detected: no
- Total interactive elements: 1
- Elements clicked: 1
- P0: 0
- P1: 0
- P2: 6
