# Audit: Firewall
URL: http://localhost:1420/firewall
Audited at: 2026-04-09T21:27:00+01:00
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
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2042 clientWidth=1888 [OVERFLOW +154px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=2020 clientWidth=1577 [OVERFLOW +443px]

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1351 clientWidth=1248 [OVERFLOW +103px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1250 clientWidth=937 [OVERFLOW +313px]

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1075 clientWidth=992 [OVERFLOW +83px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Overview | button | — | yes |
| 2 | Pattern Library | button | — | yes |

## Click sequence
### Click 1: "Overview"
- Pathname before: /firewall
- New console: clean
- Network failures: none
- Visible change: none — "Overview" was already the active tab; content unchanged ("Connect to desktop runtime to view firewall status.")
- Pathname after: /firewall
- Reverted: n/a

### Click 2: "Pattern Library"
- Pathname before: /firewall
- New console: clean
- Network failures: none
- Visible change: "Pattern Library" button gained active style (teal bg `rgb(20, 184, 166)`, white text); "Overview" button lost active style. Content area text unchanged — still shows "Connect to desktop runtime to view firewall status."
- Pathname after: /firewall
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 2
### Elements clicked: 2 (of 2)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0 (all buttons have text content)
- Additional a11y notes:
  - Both tab buttons in `main` missing `type` attribute (defaults to `submit`)
  - Tab buttons lack `role="tab"` and `aria-selected` attributes
  - Tab container `div` lacks `role="tablist"`
  - H1 "Firewall" is in the shell `header` element, not inside `main`

## Findings

### firewall-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows at every viewport: 2042 vs 1888 at 1920x1080, 1351 vs 1248 at 1280x800, 1075 vs 992 at 1024x768. This is the known shell-level background overflow (consistent with audit, timeline, trust pages).
- IMPACT: Cosmetic; no visible scrollbar appears because parent clips, but the element extends beyond the viewport boundary.

### firewall-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920|1280
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` overflows: scrollWidth=2020 vs clientWidth=1577 at 1920x1080 (+443px), scrollWidth=1250 vs clientWidth=937 at 1280x800 (+313px). Not detected at 1024x768 (panel may reflow or be hidden).
- IMPACT: Known holo-panel overflow pattern; content clipped by `overflow:hidden` on parent but panel dimensions are incorrect.

### firewall-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Tab buttons "Overview" and "Pattern Library" in `main` lack `role="tab"`, `aria-selected`, and their parent `div` lacks `role="tablist"`. Buttons also missing explicit `type="button"` attribute (default is `submit`).
- IMPACT: Screen readers cannot convey tab semantics; users relying on assistive technology cannot determine which tab is active or navigate the tabbed interface.

### firewall-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The page `h1` "Firewall" is inside the shell `header` element, not inside `main`. The highest heading inside `main` is `h2` "Prompt Firewall".
- IMPACT: Landmark-based navigation tools may not associate the primary heading with the main content region.

### firewall-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Both "Overview" and "Pattern Library" tabs display identical content: "Connect to desktop runtime to view firewall status." Tab switching updates visual active state (teal highlight moves) but content area does not change.
- IMPACT: Tabs appear functional (active state toggles) but are effectively no-ops in demo mode — no mock data is provided for either tab, making the tabbed interface misleading.

## Summary
- Gate detected: no
- Total interactive elements: 2
- Elements clicked: 2
- P0: 0
- P1: 0
- P2: 5
