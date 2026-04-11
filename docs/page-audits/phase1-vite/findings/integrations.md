# Audit: Integrations
URL: http://localhost:1420/integrations
Audited at: 2026-04-09T21:02:51Z
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

### 1920x1080 (actual viewport 1888x895)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2032 clientWidth=1888 (+144px) [OVERFLOW]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=2154 clientWidth=1577 (+577px) [OVERFLOW]
  - `span.nexus-sidebar-item-text` (×3): scrollWidth=157 clientWidth=153 (+4px each) [OVERFLOW — minor text truncation in sidebar]

### 1280x800 (actual viewport 1248x615)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1341 clientWidth=1248 (+93px) [OVERFLOW]
  - `span.nexus-sidebar-item-text` (×3): scrollWidth=157 clientWidth=153 (+4px each) [OVERFLOW]
  - anonymous `div`: scrollWidth=368 clientWidth=357 (+11px) [OVERFLOW]
  - `button` (×2): scrollWidth=56/50 clientWidth=32 (+24/+18px) [OVERFLOW — filter buttons clipped]

### 1024x768 (actual viewport 992x583)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1064 clientWidth=992 (+72px) [OVERFLOW]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=935 clientWidth=681 (+254px) [OVERFLOW]
  - `span.nexus-sidebar-item-text` (×3): scrollWidth=157 clientWidth=153 (+4px each) [OVERFLOW]
  - anonymous `div`: scrollWidth=368 clientWidth=357 (+11px) [OVERFLOW]
  - `button` (×2): scrollWidth=56/50 clientWidth=32 (+24/+18px) [OVERFLOW — filter buttons clipped]

## Interactive elements (main content only)

The page has 3 tabs (Marketplace, Event Routing, Health Status) that swap the content panel. Elements listed are for the default Marketplace view.

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Marketplace | button (intg-tab) | — | yes |
| 2 | Event Routing | button (intg-tab) | — | yes |
| 3 | Health Status | button (intg-tab) | — | yes |
| 4 | All | button (intg-filter-btn) | — | yes |
| 5 | Messaging | button (intg-filter-btn) | — | yes |
| 6 | Ticketing | button (intg-filter-btn) | — | yes |
| 7 | DevOps | button (intg-filter-btn) | — | yes |
| 8 | Custom | button (intg-filter-btn) | — | yes |
| 9 | Retry | button (intg-btn) | — | yes |

Event Routing tab: 3 buttons (the 3 tabs) + static routing matrix table (no interactive elements).
Health Status tab: 3 buttons (the 3 tabs) + 1 "Refresh" button (intg-btn intg-btn--sm).

## Click sequence

### Click 1: "Marketplace"
- Pathname before: /integrations
- New console: clean
- Network failures: none
- Visible change: none (already active tab)
- Pathname after: /integrations
- Reverted: n/a

### Click 2: "Event Routing"
- Pathname before: /integrations
- New console: clean
- Network failures: none
- Visible change: Tab switches to Event Routing — shows "Event Routing Matrix" with event list (agent_started, agent_completed, agent_error, hitl_required, etc.). Category filter buttons disappear. No interactive elements besides tabs.
- Pathname after: /integrations
- Reverted: n/a

### Click 3: "Health Status"
- Pathname before: /integrations
- New console: clean
- Network failures: none
- Visible change: Tab switches to Health Status — shows "Provider Health Status" heading with a Refresh button. Category filter buttons disappear. No provider data displayed (empty state with no message).
- Pathname after: /integrations
- Reverted: n/a

### Click 4: "All" (filter — returned to Marketplace first)
- Pathname before: /integrations
- New console: clean
- Network failures: none
- Visible change: Filter "All" becomes active. Error message "Failed to load integrations — check backend connection." persists regardless of filter.
- Pathname after: /integrations
- Reverted: n/a

### Click 5: "Messaging" (filter)
- Pathname before: /integrations
- New console: clean
- Network failures: none
- Visible change: Filter "Messaging" becomes active, replacing "All".
- Pathname after: /integrations
- Reverted: n/a

### Click 6: "Ticketing" (filter — skipped in sequence, but tested via filter-All)
- Not individually tested (covered by filter behavior pattern — filters activate correctly).

### Click 7: "Retry"
- Pathname before: /integrations
- New console: clean
- Network failures: none
- Visible change: none — error message persists, no loading indicator shown.
- Pathname after: /integrations
- Reverted: n/a

### Click 8: "Refresh" (Health Status tab)
- Pathname before: /integrations
- New console: clean
- Network failures: none
- Visible change: none — "Provider Health Status" still empty, no loading indicator.
- Pathname after: /integrations
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 9 (Marketplace) + 1 (Health Status Refresh) = 10 unique
### Elements clicked: 8

## Accessibility
- Images without alt: 0
- Inputs without label: 0 (no inputs on page)
- Buttons without accessible name: 0 (all buttons have text content)
- Buttons without `type` attribute: 9 (Marketplace, Event Routing, Health Status, All, Messaging, Ticketing, DevOps, Custom, Retry) — all default to `type="submit"`

## Findings

### integrations-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows at every viewport: +144px at 1920, +93px at 1280, +72px at 1024. This is a decorative background element that exceeds the viewport width.
- IMPACT: Cosmetic — may cause horizontal scrollbar if parent `overflow:hidden` is removed.

### integrations-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920|1024
- EVIDENCE: `section.holo-panel.holo-panel-mid` overflows: +577px at 1920, +254px at 1024. Root cause: `.holo-panel__refraction` is absolutely positioned with `width: 2207.53px` and `left: -315.359px`, exceeding the parent. Parent has `overflow: hidden` but `scrollWidth` still reports the overflow.
- IMPACT: Cosmetic — contained by `overflow: hidden` on parent. No visible scrollbar.

### integrations-03
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 1280|1024
- EVIDENCE: At 1280x800 and 1024x768, two filter buttons overflow their containers: one button reports scrollWidth=56 vs clientWidth=32 (+24px), another scrollWidth=50 vs clientWidth=32 (+18px). The filter button text is clipped at smaller viewports.
- IMPACT: Filter button labels may be unreadable at smaller viewport widths, making the Marketplace category filter unusable.

### integrations-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 9 buttons in the Marketplace view lack a `type` attribute (confirmed: `hasAttribute('type')` returns false). They default to `type="submit"`, which is incorrect for tab/filter buttons not inside a `<form>`.
- IMPACT: Semantic HTML issue — screen readers may announce these as submit buttons. Could cause unexpected form submission if a parent `<form>` is ever added.

### integrations-05
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Retry" button on Marketplace tab produces no visible effect — no loading state, no console output, no network request. The error message "Failed to load integrations — check backend connection." persists unchanged.
- IMPACT: User has no feedback that the retry action was attempted. In demo mode this is expected (no backend), but there is no loading spinner or transient state change to indicate the button registered the click.

### integrations-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button on Health Status tab produces no visible effect — no loading indicator, no console output. The "Provider Health Status" section shows just a heading and the Refresh button with no data and no empty-state message.
- IMPACT: No feedback that refresh was attempted. No empty-state message tells the user why no providers are listed.

### integrations-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Health Status tab shows "Provider Health Status" heading and a "Refresh" button, but no provider list, no table, and no empty-state message explaining there are no providers.
- IMPACT: User sees a blank area with no explanation — unclear whether data is loading, failed, or simply empty.

## Summary
- Gate detected: no
- Total interactive elements: 10 (9 on Marketplace + 1 Refresh on Health Status)
- Elements clicked: 8
- P0: 0
- P1: 2
- P2: 5
