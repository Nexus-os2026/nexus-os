# Audit: Temporal
URL: http://localhost:1420/temporal
Audited at: 2026-04-10T00:02:00Z
Gate detected: false
Gate type: none

## Console (captured at page load, ALL messages)
### Errors
none
### Warnings
none
### Logs
none
### Info
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js:21550:24
### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

Note: `resize_window` consistently fails to change the browser viewport. All three target viewports (1920x1080, 1280x800, 1024x768) were requested but the viewport remained locked at 992x639. Measurements below are at the actual 992x639 viewport.

### 992x639 (actual — requested 1920x1080)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1071 clientWidth=992 [OVERFLOW +79px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=741 clientWidth=681 [OVERFLOW +60px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px, minor text truncation]

### 1280x800
- Resize failed — viewport remained at 992x639. See above.

### 1024x768
- Resize failed — viewport remained at 992x639. See above.

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button[type=submit] | — | true |
| 2 | Start Jarvis | button[type=submit] | — | true |
| 3 | TIMELINES | button[type=button] | — | true |
| 4 | NEW FORK | button[type=button] | — | true |
| 5 | DILATED SESSION | button[type=button] | — | true |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /temporal
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /temporal
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /temporal
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /temporal
- Reverted: n/a

### Click 3: "TIMELINES"
- Pathname before: /temporal
- New console: clean
- Network failures: none
- Visible change: none — tab button has no visible active state change (bgColor stays transparent), content panel unchanged ("Timeline Tree / No temporal forks yet")
- Pathname after: /temporal
- Reverted: n/a

### Click 4: "NEW FORK"
- Pathname before: /temporal
- New console: clean
- Network failures: none
- Visible change: none — content panel unchanged, same "Timeline Tree" content displayed
- Pathname after: /temporal
- Reverted: n/a

### Click 5: "DILATED SESSION"
- Pathname before: /temporal
- New console: clean
- Network failures: none
- Visible change: tab button background changes to cyan (rgb(34,211,238)) — active state visual update. However content panel remains unchanged ("Timeline Tree / No temporal forks yet")
- Pathname after: /temporal
- Reverted: n/a

### Skipped (destructive)
- none

### Total interactive elements found: 5
### Elements clicked: 5 (all clicked)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Duplicate H1: 2 — "Temporal Engine" (in banner header) and "TEMPORAL ENGINE" (in main)
- Nav without aria-label: 1 — `<nav>` in sidebar has no `aria-label`
- No `[role="banner"]` landmark declared
- Tab buttons missing `role="tab"` and `aria-selected` — active state is CSS-only, not communicated to assistive technology

## Findings

### temporal-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 992 (resize failed for all targets)
- EVIDENCE: `div.living-background` scrollWidth=1071 > clientWidth=992 (+79px); `section.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=741 > clientWidth=681 (+60px)
- IMPACT: Background and holo-panel overflow their containers horizontally, may cause layout shift or hidden content on smaller viewports

### temporal-02
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (type=submit) clicked — zero console output, zero network requests, zero visible change
- IMPACT: Button appears clickable but performs no action in demo mode; no loading state or disabled attribute communicates this to user

### temporal-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button (type=submit) clicked — zero console output, zero network requests, zero visible change
- IMPACT: Button appears clickable but performs no action in demo mode; no feedback provided

### temporal-04
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Tab buttons "TIMELINES", "NEW FORK", "DILATED SESSION" update visual active state (DILATED SESSION gets cyan bg) but content panel never changes — always shows "Timeline Tree / No temporal forks yet" regardless of selected tab
- IMPACT: Tabs are broken — user expects tab selection to switch content panels but nothing changes; the "NEW FORK" and "DILATED SESSION" views are unreachable

### temporal-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on page: "Temporal Engine" in banner (parent: `div.flex.flex-wrap.items-center.gap-2.5`) and "TEMPORAL ENGINE" in `<main>` (parent: `div`)
- IMPACT: Screen readers announce two top-level headings, creating ambiguous document structure

### temporal-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Tab buttons have no `role="tab"`, no `aria-selected`, no `role="tablist"` on parent. Active tab state is CSS-only (inline backgroundColor change)
- IMPACT: Screen readers cannot identify tabs or their selection state; keyboard navigation pattern (arrow keys) not available

### temporal-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<nav>` element inside `aside.nexus-sidebar-shell` has no `aria-label` or `aria-labelledby` attribute
- IMPACT: Screen readers announce an unlabeled navigation landmark, making it harder to distinguish from other nav regions

## Summary
- Gate detected: no
- Total interactive elements: 5
- Elements clicked: 5
- P0: 0
- P1: 1
- P2: 6
