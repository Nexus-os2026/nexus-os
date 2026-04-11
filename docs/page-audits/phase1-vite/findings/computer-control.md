# Audit: Computer Control
URL: http://localhost:1420/computer-control
Audited at: 2026-04-10T00:12:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
- `[Nexus OS] Unhandled promise rejection: Error: desktop runtime unavailable` at src/main.tsx:41:10 — triggered by "I Understand — Enable Live Control" button click, stack: `invokeDesktop` (src/api/backend.ts:17:11) -> `invokeJsonDesktop` (src/api/backend.ts:22:25) -> `computerControlToggle` (src/api/backend.ts:222:10) -> `onClick` (src/pages/ComputerControl.tsx:425:86)

### Warnings
none

### Logs
none

### Info
- `Download the React DevTools for a better development experience: https://reactjs.org/link/react-devtools` at chunk-NUMECXU6.js:21550:24

### Debug
- `[vite] connecting...` at @vite/client:494:8
- `[vite] connected.` at @vite/client:617:14

## Overflow

### 1920x1080
NOTE: resize_window tool did not change JS-reported viewport; measurements taken at actual 1888x951.

- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px, text truncation]

### 1280x800
NOT MEASURED — resize_window tool did not change viewport from 1888x951.

### 1024x768
NOT MEASURED — resize_window tool did not change viewport from 1888x951.

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | yes |
| 2 | Start Jarvis | button | — | yes |
| 3 | Enable | button | — | yes |
| 4 | Kill Switch | button | — | yes |
| 5 | Preview Mode | button | — | yes |
| 6 | Live Mode | button | — | yes |
| 7 | I Understand — Enable Live Control | button | — | yes |
| 8 | (action prompt textarea) | textarea | — | yes |
| 9 | Start Action | button | — | yes |
| 10 | Enable (Omniscience) | button | — | yes |
| 11 | Refresh (Screen Context) | button | — | yes |
| 12 | Refresh (Intent Predictions) | button | — | yes |
| 13 | (app name input) | input[text] | — | yes |
| 14 | Get Context | button | — | yes |
| 15 | (JSON action textarea) | textarea | — | yes |
| 16 | Execute | button | — | yes |

## Click sequence
### Click 1: "Refresh" (header)
- Pathname before: /computer-control
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /computer-control
- Reverted: n/a

### Click 2: "Start Jarvis" (header)
- Pathname before: /computer-control
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /computer-control
- Reverted: n/a

### Click 3: "Enable" (Computer Control)
- Pathname before: /computer-control
- New console: clean
- Network failures: none
- Visible change: none — heading remains "Computer Control: OFF", button still says "Enable"
- Pathname after: /computer-control
- Reverted: n/a

### Click 4: "Kill Switch"
- Pathname before: /computer-control
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /computer-control
- Reverted: n/a

### Click 5: "Preview Mode" (tab)
- Pathname before: /computer-control
- New console: clean
- Network failures: none
- Visible change: Preview Mode tab gains active backgroundColor, already was default view — no content change
- Pathname after: /computer-control
- Reverted: n/a

### Click 6: "Live Mode" (tab)
- Pathname before: /computer-control
- New console: clean
- Network failures: none
- Visible change: Live Mode tab gains rgba(239, 68, 68, 0.2) backgroundColor; content area shows "Live Mode" h3 and "I Understand — Enable Live Control" button. Tab switching works correctly.
- Pathname after: /computer-control
- Reverted: n/a

### Click 7: "I Understand — Enable Live Control"
- Pathname before: /computer-control
- New console: ERROR — `[Nexus OS] Unhandled promise rejection: Error: desktop runtime unavailable` at src/main.tsx:41:10, via computerControlToggle at src/api/backend.ts:222
- Network failures: none
- Visible change: Button disappeared from DOM after click; replaced by "Run Preview" in all tab views. State corrupted — Live Mode content no longer shows its original gate button.
- Pathname after: /computer-control
- Reverted: n/a

### Click 8: "Start Action"
- Pathname before: /computer-control
- New console: clean
- Network failures: none
- Visible change: none — silent no-op (textarea had default placeholder text)
- Pathname after: /computer-control
- Reverted: n/a

### Click 9: "Enable" (Omniscience)
- Pathname before: /computer-control
- New console: clean
- Network failures: none
- Visible change: none — heading remains "Omniscience: OFF", button still says "Enable"
- Pathname after: /computer-control
- Reverted: n/a

### Click 10: "Refresh" (Screen Context)
- Pathname before: /computer-control
- New console: clean
- Network failures: none
- Visible change: none — still shows "No screen context captured yet."
- Pathname after: /computer-control
- Reverted: n/a

### Skipped (destructive)
none — no destructive keywords found in any button labels.

### Total interactive elements found: 16
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 3 (selectors: `textarea.mt-3.w-full` (action prompt), `input.flex-1.rounded-xl[placeholder="App name (e.g. Firefox)"]`, `textarea.mt-3.w-full[placeholder='{"type":"click","x":100,"y":200}']`)
- Buttons without accessible name: 0
- Additional: nav.nexus-sidebar-nav has no aria-label; 2 `<header>` landmarks outside sidebar (duplicate banner)

## Findings

### computer-control-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: Clicking "I Understand — Enable Live Control" button triggers unhandled promise rejection: `Error: desktop runtime unavailable` at `computerControlToggle` (src/api/backend.ts:222) called from onClick (src/pages/ComputerControl.tsx:425). Error bubbles to global handler at src/main.tsx:41.
- IMPACT: Live Control toggle crashes silently in demo mode; button disappears from DOM after failed toggle, corrupting the Live Mode tab UI state.

### computer-control-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `div.living-background` scrollWidth=2039 > clientWidth=1888 (+151px). `section.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=1716 > clientWidth=1577 (+139px). Both overflow horizontally at 1888x951.
- IMPACT: Horizontal scrollbar or hidden content bleed; background element extends 151px beyond viewport.

### computer-control-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: 5 buttons missing `type="button"` attribute: "Refresh" (header), "Start Jarvis" (header), "Preview Mode", "Live Mode", "I Understand — Enable Live Control". Verified via `btn.getAttribute('type')` returning null.
- IMPACT: Buttons default to `type="submit"` per HTML spec; could cause unintended form submission if wrapped in a `<form>` ancestor.

### computer-control-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 3 inputs lack associated label or aria-label: (1) `textarea.mt-3.w-full` (action prompt, has placeholder only), (2) `input.flex-1.rounded-xl[placeholder="App name (e.g. Firefox)"]`, (3) `textarea.mt-3.w-full[placeholder='{"type":"click","x":100,"y":200}']`.
- IMPACT: Screen readers cannot announce the purpose of these form fields.

### computer-control-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<header>` elements exist outside sidebar: `header.nexus-shell-header` (page header) and `header.nexus-panel` (Computer Control banner). Both implicitly create `banner` landmark. `nav.nexus-sidebar-nav` has no `aria-label`.
- IMPACT: Duplicate banner landmarks confuse screen reader landmark navigation; unlabeled nav is ambiguous.

### computer-control-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" (header) and "Start Jarvis" (header) buttons produce no console output, no network request, no visible change, and no navigation on click. Both are missing `type="button"`.
- IMPACT: Buttons appear functional but are completely inert — misleading interactive surface.

### computer-control-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Enable" (Computer Control) button click produces no state change — heading remains "Computer Control: OFF", button label unchanged, no console output. "Kill Switch" button is similarly inert. "Enable" (Omniscience) button click produces no change — heading remains "Omniscience: OFF".
- IMPACT: Primary power controls appear functional but do nothing in demo mode, with no user feedback.

## Summary
- Gate detected: no
- Total interactive elements: 16
- Elements clicked: 10
- P0: 0
- P1: 2
- P2: 5
