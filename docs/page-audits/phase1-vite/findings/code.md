# Audit: Code
URL: http://localhost:1420/code
Audited at: 2026-04-09T20:10:00Z
Gate detected: true
Gate type: RequiresLlm

## Console (captured at 1248x671, ALL messages)
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

Note: Browser window is not resizable via MCP tool or script. Actual viewport is 1248x671. Viewports 1920x1080 and 1280x800 are reported at native viewport; 1024x768 is simulated via CSS `max-width` constraint on `documentElement`.

### 1920x1080 (measured at native 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `div.nexus-main-column`: scrollWidth=990 clientWidth=990 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]

### 1280x800 (measured at native 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `div.nexus-main-column`: scrollWidth=990 clientWidth=990 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]

### 1024x768 (simulated via CSS max-width)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1024 clientWidth=1024 [OK]
- main `div.nexus-main-column`: scrollWidth=766 clientWidth=766 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=776 clientWidth=713 [OVERFLOW +63px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | yes |
| 2 | Start Jarvis | button | — | yes |
| 3 | Install Ollama (Free, Local, Private) | button | — | yes |
| 4 | I have an API key (OpenAI, Anthropic, etc.) | a | #/settings | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /code
- New console: clean
- Network failures: none
- Visible change: none — button is a silent no-op in demo mode
- Pathname after: /code
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /code
- New console: clean
- Network failures: none
- Visible change: none — button is a silent no-op in demo mode
- Pathname after: /code
- Reverted: n/a

### Click 3: "Install Ollama (Free, Local, Private)"
- Pathname before: /code
- New console: clean
- Network failures: none
- Visible change: none — `onClick` guarded by `hasDesktopRuntime()` which returns false in demo mode; confirmed in source RequiresLlm.tsx:66
- Pathname after: /code
- Reverted: n/a

### Click 4: "I have an API key (OpenAI, Anthropic, etc.)"
- Pathname before: /code
- New console: clean
- Network failures: none
- Visible change: none — hash changed to `#/settings` but no navigation occurred; page content unchanged
- Pathname after: /code (hash: #/settings)
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 4
### Elements clicked: 4

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- ARIA roles in main content: 0 (no ARIA landmarks, regions, or widget roles found anywhere in main content area)

## Findings

### code-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` has `position: fixed` and `scrollWidth=1348` vs `clientWidth=1248` at native viewport — 100px horizontal overflow. Persists across all measured viewports.
- IMPACT: Background element extends beyond viewport, potentially causing horizontal scrollbar or clipped decorative content.

### code-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has `overflow: hidden` and `scrollWidth=1019` vs `clientWidth=937` at native viewport (+82px). At 1024 simulated: sw=776 cw=713 (+63px). Content is silently clipped.
- IMPACT: Gate card content may be clipped at narrower viewports without any scroll affordance; user cannot reach overflowing content.

### code-03
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "I have an API key" link in RequiresLlm.tsx:94 uses `href="#/settings"` which sets `window.location.hash` to `#/settings` but does not navigate to `/settings`. The app uses React Router (pathname-based routing), not hash-based routing. Page content is unchanged after click.
- IMPACT: Users who click "I have an API key" expecting to reach the Settings page are stranded on the same page with no visible feedback.

### code-04
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Install Ollama" button in RequiresLlm.tsx:64-71 has `onClick` handler guarded by `hasDesktopRuntime()`. In demo mode this returns false, so the click is a silent no-op — no console message, no toast, no visual feedback.
- IMPACT: Users in demo/browser mode get zero feedback when clicking the primary CTA button on the gate screen.

### code-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (App.tsx status bar) fires silently in demo mode — no console output, no visual change, no loading indicator.
- IMPACT: Button appears interactive but provides no feedback, violating user expectations.

### code-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button (App.tsx status bar) fires silently in demo mode — no console output, no visual change, no loading indicator.
- IMPACT: Button appears interactive but provides no feedback, violating user expectations.

### code-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Zero ARIA roles found in the entire main content area (`div.nexus-main-column`). The RequiresLlm gate card has no `role="dialog"` or `role="alert"`. The status bar has no `role="status"`. Page header has no `role="banner"`. No landmark roles at all.
- IMPACT: Screen readers cannot identify page regions or the gate card as a distinct semantic section.

## Summary
- Gate detected: yes
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 3
- P2: 4
