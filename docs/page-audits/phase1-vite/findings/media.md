# Audit: Media
URL: http://localhost:1420/media
Audited at: 2026-04-09T23:28:44Z
Gate detected: true
Gate type: RequiresLlm

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

### 1920x1080 (measured at 1888x951 — resize_window cannot change viewport)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px, masked by overflow:hidden]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

### 1280x800 (viewport stayed at 1888x951 — resize_window had no effect)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as 1920x1080 (viewport did not change)

### 1024x768 (viewport stayed at 1888x951 — resize_window had no effect)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as 1920x1080 (viewport did not change)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | yes |
| 2 | Start Jarvis | button | — | yes |
| 3 | Install Ollama (Free, Local, Private) | button | — | yes |
| 4 | I have an API key (OpenAI, Anthropic, etc.) | a | #/settings | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /media
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /media
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /media
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /media
- Reverted: n/a

### Click 3: "Install Ollama (Free, Local, Private)"
- Pathname before: /media
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /media
- Reverted: n/a

### Click 4: "I have an API key (OpenAI, Anthropic, etc.)"
- Pathname before: /media
- New console: clean
- Network failures: none
- Visible change: URL changed to /media#/settings; gate screen remained unchanged — no navigation to settings page
- Pathname after: /media (hash changed to #/settings)
- Reverted: yes (navigated back to /media)

### Skipped (destructive)
none

### Total interactive elements found: 4
### Elements clicked: 4 (of 4)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Buttons missing `type` attribute: 3 (`Refresh`, `Start Jarvis`, `Install Ollama (Free, Local, Private)`)
- Header element lacks `role="banner"`
- Gate `section.holo-panel.holo-panel-mid` lacks `role` and `aria-label` attributes

## Findings

### media-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 1920 (1888 actual)
- EVIDENCE: `div.living-background` has scrollWidth=2039 vs clientWidth=1888 — overflows viewport by 151px horizontally
- IMPACT: Background element extends beyond viewport, may cause horizontal scrollbar on some browsers or clip visible content

### media-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920 (1888 actual)
- EVIDENCE: `section.holo-panel.holo-panel-mid` has scrollWidth=1716 vs clientWidth=1577 (+139px overflow), masked by computed `overflow:hidden`
- IMPACT: Internal content is clipped silently; overflow:hidden masks a real layout bug inside the holo-panel

### media-03
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button click produces no console output, no network request, no visible change. Silent no-op in demo mode.
- IMPACT: User clicks a visibly enabled button and gets zero feedback — violates user expectation

### media-04
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button click produces no console output, no network request, no visible change. Silent no-op in demo mode.
- IMPACT: User clicks a visibly enabled button and gets zero feedback — violates user expectation

### media-05
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Install Ollama" gate button click produces no console output, no network request, no visible change. Silent no-op in demo mode.
- IMPACT: Primary CTA on gate screen does nothing — user has no path to proceed

### media-06
- SEVERITY: P1
- DIMENSION: gate
- VIEWPORT: all
- EVIDENCE: "I have an API key" link uses `href="#/settings"` which navigates to `/media#/settings` instead of `/settings`. Gate screen remains unchanged after click.
- IMPACT: Broken navigation — user cannot reach settings page to configure an API key; gate is a dead end

### media-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Three buttons in main content area (`Refresh`, `Start Jarvis`, `Install Ollama`) lack `type` attribute. Default type is "submit" which can cause unintended form submissions.
- IMPACT: Accessibility and semantic correctness issue; buttons should have explicit `type="button"`

### media-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Gate section `section.holo-panel.holo-panel-mid` has no `role` or `aria-label`. Header element lacks `role="banner"`.
- IMPACT: Screen readers cannot identify the gate region or header landmark, reducing navigability

## Summary
- Gate detected: yes
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 5
- P2: 3
