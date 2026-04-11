# Audit: Measurement Session
URL: http://localhost:1420/measurement-session
Audited at: 2026-04-09T22:08:00+01:00
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
- `Download the React DevTools for a better development experience: https://reactjs.org/link/react-devtools` — chunk-NUMECXU6.js?v=5144749d:21550:24

### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=2025 clientWidth=1577 [OVERFLOW +448px]
  - `div.living-background`: scrollWidth=2034 clientWidth=1888 [OVERFLOW +146px]
  - `div.holo-panel__refraction`: rendered width=2287px (decorative element inside overflow:hidden panel)

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1185 clientWidth=937 [OVERFLOW +248px]
  - `div.living-background`: scrollWidth=1342 clientWidth=1248 [OVERFLOW +94px]

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1066 clientWidth=992 [OVERFLOW +74px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (nx-btn nx-btn-ghost) | — | yes |
| 2 | Start Jarvis | button (nx-btn nx-btn-primary) | — | yes |

Note: Both buttons are in the page header/banner area (`[role="banner"]`), outside `<main>`. There are zero interactive elements inside `<main>` itself. The main content contains only a static error div: "Error: desktop runtime unavailable".

## Click sequence
### Click 1: "Refresh"
- Pathname before: /measurement-session
- New console: clean (no messages)
- Network failures: none
- Visible change: none — button is a silent no-op in demo mode
- Pathname after: /measurement-session
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /measurement-session
- New console: clean (no messages)
- Network failures: none
- Visible change: none — button is a silent no-op in demo mode
- Pathname after: /measurement-session
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 2 (both in header, 0 in main)
### Elements clicked: 2

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- h1 "Session Detail" is outside `<main>`, positioned 116px above it in the banner area
- 2 page-content buttons missing `type` attribute: "Refresh", "Start Jarvis"
- Error div `"Error: desktop runtime unavailable"` in main lacks `role="alert"` and `aria-live` attribute

## Findings

### measurement-session-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has `overflow:hidden` and clips content. At 1920x1080: scrollWidth=2025 vs clientWidth=1577 (+448px horizontal), scrollHeight=1023 vs clientHeight=637 (+386px vertical). `div.holo-panel__refraction` renders at 2287px wide. At 1280x800: +248px overflow. At 1024x768 the holo-panel overflow collapses but `div.living-background` still overflows by 74–146px across all viewports.
- IMPACT: Decorative holo-panel clips page content vertically (386px hidden at 1920x1080) and overflows horizontally at all tested viewports.

### measurement-session-02
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Clicking "Refresh" (button.nx-btn-ghost) and "Start Jarvis" (button.nx-btn-primary) in the page header produces no console output, no network requests, and no visible UI change. Both are silent no-ops in demo mode.
- IMPACT: Users see interactive-looking buttons that do nothing; no feedback or disabled state communicates that desktop runtime is required.

### measurement-session-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<h1>Session Detail</h1>` is inside `[role="banner"]` (ref_211), positioned 116px above `<main>` (ref_220). The h1 is not contained within any landmark that screen readers associate with primary page content.
- IMPACT: Screen readers navigating by landmark may skip the page heading when jumping to `<main>`.

### measurement-session-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons lack `type` attribute. Both default to `type="submit"`, which is incorrect for action buttons not inside a `<form>`.
- IMPACT: Buttons may trigger unintended form submission behavior if wrapped in a form in the future; violates HTML best practice.

### measurement-session-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The error message `"Error: desktop runtime unavailable"` inside `<main>` is rendered in a plain `<div>` with no `role="alert"` and no `aria-live` attribute.
- IMPACT: Assistive technologies will not announce this error state to users; the primary page content is an unannounced error.

### measurement-session-06
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows the viewport at all three breakpoints: +146px at 1920x1080, +94px at 1280x800, +74px at 1024x768. The documentElement and body do not expose scrollbars (the overflow is contained), but the decorative background extends beyond viewport bounds.
- IMPACT: While scrollbars are suppressed, the background element wastes rendering area and could cause layout issues if overflow containment changes.

## Summary
- Gate detected: no
- Total interactive elements: 2 (both in header, 0 in main)
- Elements clicked: 2
- P0: 0
- P1: 2
- P2: 4
