# Audit: Dreams
URL: http://localhost:1420/dreams
Audited at: 2026-04-09T23:32:00Z
Gate detected: true
Gate type: RequiresLlm

## Console (captured at 992x639, ALL messages)
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

NOTE: `resize_window` MCP tool reported success for all three target viewports but `window.innerWidth/Height` remained fixed at 992x639. All measurements below are at the actual 992x639 viewport.

### 992x639 (actual viewport, requested 1920x1080)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1071 clientWidth=992 [OVERFLOW +79px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=741 clientWidth=681 [OVERFLOW +60px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

### 1280x800
- Not measurable — resize_window had no effect (viewport stayed 992x639)

### 1024x768
- Not measurable — resize_window had no effect (viewport stayed 992x639)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | true |
| 2 | Start Jarvis | button | — | true |
| 3 | Install Ollama (Free, Local, Private) | button | — | true |
| 4 | I have an API key (OpenAI, Anthropic, etc.) | a | #/settings | true |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /dreams
- New console: clean
- Network failures: none
- Visible change: none — button is a silent no-op
- Pathname after: /dreams
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /dreams
- New console: clean
- Network failures: none
- Visible change: none — button is a silent no-op
- Pathname after: /dreams
- Reverted: n/a

### Click 3: "Install Ollama (Free, Local, Private)"
- Pathname before: /dreams
- New console: clean
- Network failures: none
- Visible change: none — button is a silent no-op
- Pathname after: /dreams
- Reverted: n/a

### Click 4: "I have an API key (OpenAI, Anthropic, etc.)"
- Pathname before: /dreams
- New console: clean
- Network failures: none
- Visible change: none — gate screen remains unchanged; link href="#/settings" appends hash to /dreams instead of navigating to /settings page
- Pathname after: /dreams (hash was empty; no navigation to /settings)
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 4
### Elements clicked: 4 (of 4)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Buttons missing `type` attribute: 3 ("Refresh", "Start Jarvis", "Install Ollama (Free, Local, Private)")
- `header` element lacks `role="banner"` attribute
- `section.holo-panel` lacks `role` and `aria-label` attributes
- Gate region (`[role="region"]` inside `main`) has `role="region"` but no `aria-label`

## Findings

### dreams-01
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 3 buttons ("Refresh", "Start Jarvis", "Install Ollama") produce zero feedback on click — no console output, no visible change, no loading state. Click handlers are either missing or silently swallowed.
- IMPACT: Users clicking any button get no indication the app received their input; perceived as broken.

### dreams-02
- SEVERITY: P1
- DIMENSION: gate
- VIEWPORT: all
- EVIDENCE: "I have an API key" link `<a href="#/settings">` appends hash fragment to current page (`/dreams#/settings`) instead of navigating to the /settings route. After click, pathname remains `/dreams`, hash is empty (cleared by router), and the gate screen is unchanged.
- IMPACT: Users cannot reach the settings page to configure an API key via the gate CTA.

### dreams-03
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 992x639
- EVIDENCE: `div.living-background` has scrollWidth=1071 vs clientWidth=992 (+79px overflow). Element uses `overflow:hidden` and `position:fixed`, so the overflow is masked but the element extends 79px beyond the viewport boundary.
- IMPACT: Background layer renders wider than viewport; may cause visual artifacts or layout shifts on some browsers/devices.

### dreams-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 992x639
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has scrollWidth=741 vs clientWidth=681 (+60px internal overflow). Computed `overflow:hidden` masks the content.
- IMPACT: Gate card content may be clipped at smaller viewports; internal content wider than its container.

### dreams-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 3 buttons in main content area (`Refresh`, `Start Jarvis`, `Install Ollama`) lack explicit `type` attribute. Default type is "submit" which could trigger unintended form submission if buttons are ever placed inside a `<form>`.
- IMPACT: Minor — no forms present currently, but violates HTML best practice.

### dreams-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<header>` element (`header.nexus-shell-header`) has no `role="banner"` attribute. `section.holo-panel` serving as the page panel has no `role` or `aria-label`. Gate region inside `main` has `role="region"` but no `aria-label` to describe its purpose.
- IMPACT: Screen readers cannot identify page landmarks or the purpose of the gate region.

### dreams-07
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 992x639
- EVIDENCE: 3x `span.nexus-sidebar-item-text` elements overflow by 4px each (scrollWidth=157, clientWidth=153). These are sidebar text labels being truncated.
- IMPACT: Minor — sidebar item text slightly clipped; cosmetic only.

## Summary
- Gate detected: yes
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 3
- P2: 4
