# Audit: Admin Console
URL: http://localhost:1420/admin-console
Audited at: 2026-04-10T00:27:00+01:00
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
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` (chunk-NUMECXU6.js?v=5144749d:21550:24)

### Debug
- `[vite] connecting...` (@vite/client:494:8)
- `[vite] connected.` (@vite/client:617:14)

## Overflow

### 1920x1080
NOTE: Actual viewport was 1888x951 (resize_window MCP tool did not change viewport). Measurements taken at native viewport.

- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px, sidebar text truncation]

### 1280x800
NOTE: resize_window MCP tool did not change viewport. Window remained at 1888x951. Measurements identical to 1920x1080 — deferred to Puppeteer screenshot pass.

### 1024x768
NOTE: resize_window MCP tool did not change viewport. Window remained at 1888x951. Measurements identical to 1920x1080 — deferred to Puppeteer screenshot pass.

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (header) | n/a | yes |
| 2 | Start Jarvis | button (header) | n/a | yes |
| 3 | Retry Connection | button (main) | n/a | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /admin-console
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /admin-console
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /admin-console
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /admin-console
- Reverted: n/a

### Click 3: "Retry Connection"
- Pathname before: /admin-console
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /admin-console
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 3
### Elements clicked: 3

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Duplicate H1 headings: 2 ("Admin Dashboard" in page header, "Admin Console" in main content)

## Findings

### admin-console-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` has scrollWidth=2039 vs clientWidth=1888, overflowing by 151px. This is the animated background layer exceeding the viewport boundary.
- IMPACT: Background element extends beyond viewport; may cause horizontal scrollbar on narrower viewports or when body overflow is not clipped.

### admin-console-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has scrollWidth=1716 vs clientWidth=1577, overflowing by 139px.
- IMPACT: Main content panel overflows its container, potentially clipping content or causing unexpected horizontal scroll.

### admin-console-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page: "Admin Dashboard" (in page header outside `main`) and "Admin Console" (inside `main`). Pages should have a single H1 for accessible document structure.
- IMPACT: Screen readers announce two top-level headings, confusing document hierarchy.

### admin-console-04
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: Page header H1 reads "Admin Dashboard" but the main content H1 reads "Admin Console". The URL is `/admin-console`. The sidebar nav button reads "Admin Dashboard".
- IMPACT: Inconsistent naming between header ("Admin Dashboard") and main content ("Admin Console") creates user confusion about which page they are on.

### admin-console-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button in page header is a silent no-op in demo mode. Click produces no console output, no network request, no visible change.
- IMPACT: Button appears functional but does nothing; no feedback to the user.

### admin-console-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button in page header is a silent no-op in demo mode. Click produces no console output, no network request, no visible change.
- IMPACT: Button appears functional but does nothing; no feedback to the user.

### admin-console-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Retry Connection" button in main content is a silent no-op in demo mode. Click produces no console output, no network request, no visible change. Expected: at minimum a console log or a toast indicating no backend is available.
- IMPACT: Primary CTA in the connection status section does nothing; user has no feedback that the action was attempted.

### admin-console-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 3 buttons outside the sidebar lack explicit `type` attribute: "Refresh", "Start Jarvis" (page header), "Retry Connection" (main). Buttons default to `type="submit"` which can cause unintended form submissions.
- IMPACT: Buttons without `type="button"` may trigger form submit behavior if placed inside a form element in the future.

## Summary
- Gate detected: no
- Total interactive elements: 3
- Elements clicked: 3
- P0: 0
- P1: 2
- P2: 6
