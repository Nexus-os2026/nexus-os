# Audit: Admin Compliance
URL: http://localhost:1420/admin-compliance
Audited at: 2026-04-10T00:39:43Z
Gate detected: false
Gate type: none

## Console (captured at 992x639, ALL messages)
### Errors
none

### Warnings
none

### Logs
none

### Info
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` (chunk-NUMECXU6.js:21550:24)

### Debug
- `[vite] connecting...` (@vite/client:494:8)
- `[vite] connected.` (@vite/client:617:14)

## Overflow

Note: `window.resizeTo()` is non-functional in Chrome extension context. All measurements taken at the browser's actual viewport of 992x639. Viewport-specific overflow testing deferred to Puppeteer screenshot pass.

### 992x639 (actual viewport)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1071 clientWidth=992 [OVERFLOW +79px]
  - `span.nexus-sidebar-item-text`: scrollWidth=157 clientWidth=153 [OVERFLOW +4px] (x3 instances)
  - `section.holo-panel`: scrollWidth=741 clientWidth=681 [OVERFLOW +60px]

### 1920x1080
- Not measured (viewport resize non-functional in extension context)

### 1280x800
- Not measured (viewport resize non-functional in extension context)

### 1024x768
- Not measured (viewport resize non-functional in extension context)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no type) | n/a | yes |
| 2 | Start Jarvis | button (no type) | n/a | yes |
| 3 | Export JSON | button (no type) | n/a | yes |
| 4 | Export CSV | button (no type) | n/a | yes |
| 5 | Overview | button (no type) | n/a | yes |
| 6 | EU AI Act | button (no type) | n/a | yes |
| 7 | SOC 2 | button (no type) | n/a | yes |
| 8 | Audit & Privacy | button (no type) | n/a | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /admin-compliance
- New console: clean
- Network failures: none
- Visible change: none (silent no-op in demo mode)
- Pathname after: /admin-compliance
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /admin-compliance
- New console: clean
- Network failures: none
- Visible change: none (silent no-op in demo mode)
- Pathname after: /admin-compliance
- Reverted: n/a

### Click 3: "Export JSON"
- Pathname before: /admin-compliance
- New console: clean
- Network failures: none
- Visible change: none (silent no-op in demo mode, no download triggered)
- Pathname after: /admin-compliance
- Reverted: n/a

### Click 4: "Export CSV"
- Pathname before: /admin-compliance
- New console: clean
- Network failures: none
- Visible change: none (silent no-op in demo mode, no download triggered)
- Pathname after: /admin-compliance
- Reverted: n/a

### Click 5: "Overview"
- Pathname before: /admin-compliance
- New console: clean
- Network failures: none
- Visible change: tab switches (Overview tab shows EU AI Act/SOC 2 summary cards, hash chain status, PII redactions)
- Pathname after: /admin-compliance
- Reverted: n/a

### Click 6: "EU AI Act"
- Pathname before: /admin-compliance
- New console: clean
- Network failures: none
- Visible change: tab switches (EU AI Act tab content displayed)
- Pathname after: /admin-compliance
- Reverted: n/a

### Click 7: "SOC 2"
- Pathname before: /admin-compliance
- New console: clean
- Network failures: none
- Visible change: tab switches (SOC 2 tab content displayed)
- Pathname after: /admin-compliance
- Reverted: n/a

### Click 8: "Audit & Privacy"
- Pathname before: /admin-compliance
- New console: clean
- Network failures: none
- Visible change: tab switches to Audit & Privacy view (Audit Trail: 0 events, Chain Verified: No; Privacy & HITL: 0 redactions, 0% approval rate)
- Pathname after: /admin-compliance
- Reverted: n/a

### Skipped (destructive)
- none

### Total interactive elements found: 8
### Elements clicked: 8 (all)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### admin-compliance-01
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page. "Admin Compliance" H1 is in `div.flex.flex-wrap.items-center.gap-2.5` outside both `<main>` and `<aside>`, orphaned from any landmark. "Compliance Dashboard" H1 is inside `main` within `div.admin-shell`.
- IMPACT: Duplicate H1 violates WCAG heading hierarchy; screen readers announce two top-level headings, confusing navigation.

### admin-compliance-02
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 8 buttons (`Refresh`, `Start Jarvis`, `Export JSON`, `Export CSV`, `Overview`, `EU AI Act`, `SOC 2`, `Audit & Privacy`) lack explicit `type` attribute. Classes: `nx-btn nx-btn-ghost`, `nx-btn nx-btn-primary`, `admin-btn`, `admin-tab`. All default to `type="submit"`.
- IMPACT: Buttons without `type="button"` default to `type="submit"`, which can cause unintended form submissions if wrapped in a `<form>` element.

### admin-compliance-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 992
- EVIDENCE: `div.living-background` has scrollWidth=1071 vs clientWidth=992, overflowing by 79px horizontally.
- IMPACT: Background layer extends beyond viewport, may cause horizontal scrollbar on smaller screens.

### admin-compliance-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 992
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has scrollWidth=741 vs clientWidth=681, overflowing by 60px.
- IMPACT: Main content panel overflows its container, potentially clipping content or causing layout issues.

### admin-compliance-05
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 992
- EVIDENCE: Three `span.nexus-sidebar-item-text` elements overflow by 4px each (scrollWidth=157 vs clientWidth=153). These are sidebar items but affect overall layout.
- IMPACT: Sidebar item text is clipped at narrow viewports.

### admin-compliance-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons (classes `nx-btn-ghost` and `nx-btn-primary`) produce zero console output, zero network activity, and zero visible feedback when clicked in demo mode.
- IMPACT: Users receive no indication that these buttons are inactive in demo mode; silent no-ops erode trust in the UI.

### admin-compliance-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Export JSON" and "Export CSV" buttons (class `admin-btn`) produce zero console output, zero network activity, and no download when clicked in demo mode.
- IMPACT: Export functionality is silently inert; users clicking expect a file download or at least a toast notification.

### admin-compliance-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Tab buttons (`Overview`, `EU AI Act`, `SOC 2`, `Audit & Privacy`) with class `admin-tab` lack `role="tab"`, `aria-selected`, and `aria-controls` attributes. Active tab uses CSS class `admin-tab--active` but no ARIA state. No `role="tablist"` on container. No `role="tabpanel"` on content panels.
- IMPACT: Screen readers cannot identify these as tabs or communicate which tab is selected, violating WAI-ARIA Tabs pattern.

### admin-compliance-09
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Admin Compliance" H1 is outside both `<main>` and `<aside>` landmarks, sitting in a `div` with no landmark role. No `<header>` or `role="banner"` wrapping it (note: `hasBannerLandmark` returned true but the H1 is not inside it).
- IMPACT: Heading is orphaned from landmark structure; screen readers navigating by landmarks will miss it.

## Summary
- Gate detected: no
- Total interactive elements: 8
- Elements clicked: 8
- P0: 0
- P1: 1
- P2: 8
