# Audit: Trust
URL: http://localhost:1420/trust
Audited at: 2026-04-09T21:22:00Z
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
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2036 clientWidth=1888 [OVERFLOW +148px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=2019 clientWidth=1577 [OVERFLOW +442px] (masked by overflow:hidden)
  - `div.holo-panel__refraction`: 2208px wide (intentionally oversized decorative element)

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1345 clientWidth=1248 [OVERFLOW +97px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1269 clientWidth=937 [OVERFLOW +332px] (masked by overflow:hidden)
  - `div.holo-panel__refraction`: 1312px wide (intentionally oversized decorative element)

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1069 clientWidth=992 [OVERFLOW +77px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=865 clientWidth=681 [OVERFLOW +184px] (masked by overflow:hidden)
  - `div.holo-panel__refraction`: 953px wide (intentionally oversized decorative element)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Trust Overview | button | — | true |
| 2 | Reputation | button | — | true |

## Click sequence
### Click 1: "Trust Overview"
- Pathname before: /trust
- New console: clean
- Network failures: none
- Visible change: none (already the active tab)
- Pathname after: /trust
- Reverted: n/a

### Click 2: "Reputation"
- Pathname before: /trust
- New console: clean
- Network failures: none
- Visible change: none — active tab remains "Trust Overview", content unchanged after 500ms
- Pathname after: /trust
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 2
### Elements clicked: 2 (of 2)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### trust-01
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Clicking the "Reputation" tab button (`button.td-tab`, second in `nav.td-tabs`) does not switch the active tab. After click, `button.td-tab-active` class remains on "Trust Overview" and main content area does not change. Verified 500ms after click — not a stale-state read. The tab handler appears non-functional.
- IMPACT: Users cannot access the Reputation tab content; 50% of the page's tab surface is broken.

### trust-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` exceeds viewport at all sizes: 2036 vs 1888 (+148px at 1920), 1345 vs 1248 (+97px at 1280), 1069 vs 992 (+77px at 1024). `section.holo-panel.holo-panel-mid` also overflows internally (2019 vs 1577 at 1920) but is masked by `overflow:hidden`. The `holo-panel__refraction` element is 2208px wide at 1920 — an intentionally oversized decorative pseudo-element. No user-visible scrollbar appears because parent containers clip the overflow.
- IMPACT: Decorative background element overflows the viewport; no visible scrollbar due to clipping, but layout is technically wider than intended.

### trust-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Both tab buttons in `nav.td-tabs` are missing `type` attribute. Neither button has `type="button"`, which means they default to `type="submit"` per HTML spec. The `nav.td-tabs` container has no `role="tablist"`, the buttons have no `role="tab"` and no `aria-selected` attribute.
- IMPACT: Screen readers cannot identify these as tab controls; missing ARIA tab pattern violates WAI-ARIA Authoring Practices.

### trust-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<h1>Trust Dashboard</h1>` is inside `header.nexus-shell-header` which is outside `<main>`. The page heading is not within the main landmark, making landmark-based navigation skip the page title.
- IMPACT: Screen-reader users navigating by landmarks will not find the H1 inside the main content region.

### trust-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<header>` elements found in the page — one at `header.nexus-shell-header` (outside main) and one at `section.td-hub > header` (inside main). Multiple banner landmarks can confuse assistive technology.
- IMPACT: Ambiguous landmark structure for screen-reader users.

## Summary
- Gate detected: no
- Total interactive elements: 2
- Elements clicked: 2
- P0: 0
- P1: 1
- P2: 4
