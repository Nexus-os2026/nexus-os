# Audit: Workspaces
URL: http://localhost:1420/workspaces
Audited at: 2026-04-10T00:24:00+01:00
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

NOTE: resize_window MCP tool did not change the JavaScript-reported viewport (remained 1248x671 across all resize attempts). Measurements below are all at the actual rendered viewport of 1248x671. This is a known limitation of the audit tooling, not a page bug.

### 1920x1080 (requested; actual 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px, text truncation in sidebar — excluded from findings per sidebar exclusion]

### 1280x800 (requested; actual 1248x671)
- Same as above (viewport did not change)

### 1024x768 (requested; actual 1248x671)
- Same as above (viewport did not change)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | n/a | yes |
| 2 | Start Jarvis | button | n/a | yes |
| 3 | + Create Workspace | button | n/a | yes |
| 4 | Refresh | button | n/a | yes |

Notes:
- Elements 1-2 are in the page banner header (outside `<main>`, inside `<banner>`).
- Elements 3-4 are in the `<main>` content area.
- All 4 buttons lack an explicit `type` attribute (browser defaults to `type="submit"`).

## Click sequence
### Click 1: "Refresh" (banner)
- Pathname before: /workspaces
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /workspaces
- Reverted: n/a

### Click 2: "Start Jarvis" (banner)
- Pathname before: /workspaces
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /workspaces
- Reverted: n/a

### Click 3: "+ Create Workspace" (main)
- Pathname before: /workspaces
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /workspaces
- Reverted: n/a

### Click 4: "Refresh" (main)
- Pathname before: /workspaces
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /workspaces
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 4
### Elements clicked: 4

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Heading hierarchy: Two `<h1>` elements ("Workspaces" in banner, "Workspaces" in main) — duplicate H1

## Findings

### workspaces-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` has scrollWidth=1348 vs clientWidth=1248 (+100px overflow). This decorative background element exceeds the viewport width.
- IMPACT: Causes potential horizontal scrollbar or clipped content on the page background layer.

### workspaces-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has scrollWidth=1019 vs clientWidth=937 (+82px overflow). The page header panel overflows its container.
- IMPACT: Header banner content may be clipped or cause unintended horizontal scroll.

### workspaces-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 4 buttons outside sidebar ("Refresh" x2, "Start Jarvis", "+ Create Workspace") lack explicit `type` attribute. Browser defaults to `type="submit"`, which can cause unintended form submission if buttons are inside a `<form>`.
- IMPACT: Accessibility and semantic HTML issue; potential unintended form submission behavior.

### workspaces-04
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (banner) clicked — no console output, no navigation, no visible change, no network request. Silent no-op.
- IMPACT: Button appears interactive but does nothing in demo mode; no user feedback provided.

### workspaces-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button (banner) clicked — no console output, no navigation, no visible change, no network request. Silent no-op.
- IMPACT: Button appears interactive but does nothing in demo mode; no user feedback provided.

### workspaces-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "+ Create Workspace" button (main) clicked — no console output, no navigation, no visible change, no network request. Silent no-op. Expected: open a create workspace form/modal or show a "requires backend" message.
- IMPACT: Primary CTA button does nothing; users cannot discover workspace creation flow even in demo mode.

### workspaces-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (main) clicked — no console output, no navigation, no visible change, no network request. Silent no-op.
- IMPACT: Button appears interactive but does nothing in demo mode; no user feedback provided.

### workspaces-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page: one in the banner header ("Workspaces") and one in the main content area ("Workspaces"). Pages should have a single `<h1>`.
- IMPACT: Screen readers and SEO tools expect a single H1 per page; duplicate H1 degrades document outline semantics.

## Summary
- Gate detected: no
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 2
- P2: 6
