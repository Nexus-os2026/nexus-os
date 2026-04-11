# Audit: Cluster
URL: http://localhost:1420/cluster
Audited at: 2026-04-10T01:06:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1248x671 effective viewport)
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

> **Note:** Viewport resize via MCP `resize_window` was not honored. All three requested viewports (1920x1080, 1280x800, 1024x768) rendered at the locked effective viewport of 1248x671 on an ultrawide 3440x1440 display. Overflow measurements below are therefore identical across all three breakpoints.

### 1920x1080 (actual: 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px, sidebar — cosmetic]

### 1280x800 (actual: 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing: same as above

### 1024x768 (actual: 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing: same as above

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Discover Peers | button (type=button) | — | yes |
| 2 | (placeholder: "Task description") | input[text] | — | yes |
| 3 | (placeholder: "Agent IDs (comma sep)") | input[text] | — | yes |
| 4 | Send | button (type=button) | — | no (disabled) |
| 5 | (placeholder: "Agent ID") | input[text] | — | yes |
| 6 | (placeholder: "Target peer ID") | input[text] | — | yes |
| 7 | Migrate | button (type=button) | — | no (disabled) |

## Click sequence
### Click 1: "Discover Peers"
- Pathname before: /cluster
- New console: clean
- Network failures: none
- Visible change: Inline text "desktop runtime unavailable" appeared near the button
- Pathname after: /cluster
- Reverted: n/a

### Click 2: input "Task description"
- Pathname before: /cluster
- New console: clean
- Network failures: none
- Visible change: Input received focus
- Pathname after: /cluster
- Reverted: n/a

### Click 3: input "Agent IDs (comma sep)"
- Pathname before: /cluster
- New console: clean
- Network failures: none
- Visible change: Input received focus
- Pathname after: /cluster
- Reverted: n/a

### Click 4: "Send" (disabled)
- Pathname before: /cluster
- New console: clean
- Network failures: none
- Visible change: none (button is disabled)
- Pathname after: /cluster
- Reverted: n/a

### Click 5: input "Agent ID"
- Pathname before: /cluster
- New console: clean
- Network failures: none
- Visible change: Input received focus
- Pathname after: /cluster
- Reverted: n/a

### Click 6: input "Target peer ID"
- Pathname before: /cluster
- New console: clean
- Network failures: none
- Visible change: Input received focus
- Pathname after: /cluster
- Reverted: n/a

### Click 7: "Migrate" (disabled)
- Pathname before: /cluster
- New console: clean
- Network failures: none
- Visible change: none (button is disabled)
- Pathname after: /cluster
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 7
### Elements clicked: 7 (all)

## Accessibility
- Images without alt: 0
- Inputs without label: 4 (all four `<input type="text">` elements lack `id`, `name`, `aria-label`, and `aria-labelledby`; only `placeholder` is set — placeholders: "Task description", "Agent IDs (comma sep)", "Agent ID", "Target peer ID")
- Buttons without accessible name: 0

## Findings

### cluster-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248 (locked)
- EVIDENCE: `div.living-background` has scrollWidth=1348 vs clientWidth=1248 (+100px overflow). Hidden by parent `overflow:hidden` but creates an invisible scrollable region.
- IMPACT: Background layer extends 100px beyond viewport, may cause layout jank or unexpected scrollbar on some browsers.

### cluster-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248 (locked)
- EVIDENCE: `section.holo-panel.holo-panel-mid` has scrollWidth=1019 vs clientWidth=937 (+82px overflow). This is the main content panel inside `<main>`.
- IMPACT: Content panel overflows its container by 82px; content may be clipped or cause horizontal scroll within the panel.

### cluster-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 4 `<input type="text">` elements in main content lack `id`, `name`, `aria-label`, and `aria-labelledby`. They only have `placeholder` attributes ("Task description", "Agent IDs (comma sep)", "Agent ID", "Target peer ID"). Placeholder alone is not a valid accessible label per WCAG 2.1 SC 1.3.1.
- IMPACT: Screen readers cannot programmatically determine the purpose of these inputs; form data cannot be submitted meaningfully without `name` attributes.

### cluster-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<header>` elements exist on the page: (1) in `DIV.nexus-main-column` (shell header with "Cluster Status" H1) and (2) in `SECTION.cs-hub` (page content header with "CLUSTER STATUS // NODE HEALTH" text). Neither is a direct child of `<body>`, so neither receives the implicit ARIA `banner` landmark role. No explicit `role="banner"` is present.
- IMPACT: Assistive technologies cannot identify the page banner landmark, reducing navigation efficiency for screen reader users.

### cluster-05
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Discover Peers" button click produces no console output and no network request. In demo mode it shows inline text "desktop runtime unavailable" but emits no programmatic feedback (no console log, no ARIA live region announcement).
- IMPACT: The action result is only communicated visually; screen reader users and automated tests receive no feedback that the action was attempted.

## Summary
- Gate detected: no
- Total interactive elements: 7
- Elements clicked: 7
- P0: 0
- P1: 0
- P2: 5
