# Audit: World Simulation
URL: http://localhost:1420/world-simulation
Audited at: 2026-04-10T00:04:00+01:00
Gate detected: false
Gate type: none

## Console (captured at ~1920x1080, ALL messages)
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
(measured at actual viewport 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW — clipped by overflow:hidden + position:fixed]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW — clipped by overflow:hidden]

### 1280x800
(resize_window reported success but viewport remained at 1888x951 — known tool limitation)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as 1920x1080 (viewport did not change)

### 1024x768
(not measured — resize tool did not take effect)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button[type=submit] | — | true |
| 2 | Start Jarvis | button[type=submit] | — | true |

Note: Both buttons are in the header/banner area. The `<main>` element contains zero interactive elements. Three scenario cards ("Tech Industry Rivalry", "National Election Stress Test", "Market Crash Contagion") are rendered as static `<div>` elements with no role, tabindex, or cursor:pointer — they are not clickable.

## Click sequence
### Click 1: "Refresh"
- Pathname before: /world-simulation
- New console: clean (no messages)
- Network failures: none
- Visible change: none
- Pathname after: /world-simulation
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /world-simulation
- New console: clean (no messages)
- Network failures: none
- Visible change: none
- Pathname after: /world-simulation
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 2
### Elements clicked: 2

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

Additional:
- H1 count: 1 ("Scenario Sandbox") — located outside `<main>`, inside the banner/header
- H2 count: 1 ("Governed Parallel World Prediction") — inside `<main>`
- `<nav>` element lacks `aria-label` or `aria-labelledby`
- No explicit `role="banner"` on header (uses semantic `<header>` which maps implicitly)

## Findings

### world-simulation-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `div.living-background` scrollWidth=2039 exceeds clientWidth=1888 by 151px. Element has `overflow:hidden; position:fixed` so content is visually clipped, but the intrinsic overflow indicates the background element is wider than the viewport.
- IMPACT: Cosmetic — no visible scrollbar due to clipping, but may cause subtle layout issues on narrower viewports.

### world-simulation-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=1716 exceeds clientWidth=1577 by 139px. Element has `overflow:hidden` so content is visually clipped.
- IMPACT: Content inside the holo-panel may be truncated or cut off without user ability to scroll to it.

### world-simulation-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (ref_217) in header has `type="submit"` instead of `type="button"`. Clicking produces no console output, no network request, no visible change.
- IMPACT: Button is a silent no-op in demo mode. `type="submit"` is semantically incorrect for a non-form action button and could trigger unintended form submission if wrapped in a `<form>`.

### world-simulation-04
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button (ref_218) in header has `type="submit"` instead of `type="button"`. Clicking produces no console output, no network request, no visible change.
- IMPACT: Button is a silent no-op in demo mode. Same `type="submit"` issue as world-simulation-03.

### world-simulation-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Main content area (`<main>`) contains zero interactive elements. Three scenario cards ("Tech Industry Rivalry", "National Election Stress Test", "Market Crash Contagion") render as static `<div>` elements with no `role`, `tabindex`, `cursor:pointer`, or click handler. Text reads "Desktop runtime required to create simulations" but cards themselves appear to be templates that should be selectable.
- IMPACT: Users cannot interact with any content in the main area. Scenario cards present as actionable UI patterns (title + description) but offer no affordance.

### world-simulation-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<nav>` element (sidebar navigation, ref_5) has no `aria-label` or `aria-labelledby` attribute.
- IMPACT: Screen readers cannot distinguish this navigation region from other landmarks. WCAG 2.1 requires labels when multiple landmarks of the same type exist.

### world-simulation-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The single `<h1>` ("Scenario Sandbox") is located outside `<main>`, inside the banner/header region. The `<main>` content starts at `<h2>` ("Governed Parallel World Prediction").
- IMPACT: Screen readers navigating by heading hierarchy within `<main>` will miss the page title. The `<h1>` should be the first heading inside `<main>` for proper document outline.

## Summary
- Gate detected: no
- Total interactive elements: 2
- Elements clicked: 2
- P0: 0
- P1: 0
- P2: 7
