# Audit: Approvals
URL: http://localhost:1420/approvals
Audited at: 2026-04-09T19:38:00+01:00
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
- `Download the React DevTools for a better development experience: https://reactjs.org/link/react-devtools` — chunk-NUMECXU6.js:21550:24

### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px] (position:fixed)
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px] (overflow:hidden — content silently clipped)

### 1280x800
- documentElement: scrollWidth=1888 clientWidth=1888 [OK — actual viewport unchanged]
- body: scrollWidth=1280 clientWidth=1280 [OK]
- main `main`: scrollWidth=1019 clientWidth=1019 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2036 clientWidth=1888 [OVERFLOW +148px] (position:fixed, unaffected by viewport)
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1224 clientWidth=969 [OVERFLOW +255px]

### 1024x768
- documentElement: scrollWidth=1888 clientWidth=1888 [OK — actual viewport unchanged]
- body: scrollWidth=1024 clientWidth=1024 [OK]
- main `main`: scrollWidth=763 clientWidth=763 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2036 clientWidth=1888 [OVERFLOW +148px] (position:fixed)
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1129 clientWidth=713 [OVERFLOW +416px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button[type=submit] | — | true |
| 2 | Start Jarvis | button[type=submit] | — | true |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /approvals
- New console: clean
- Network failures: none
- Visible change: none — button click produces no observable effect
- Pathname after: /approvals
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /approvals
- New console: clean
- Network failures: none
- Visible change: none — button click produces no observable effect
- Pathname after: /approvals
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 2
### Elements clicked: 2 (of 2)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### approvals-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` has `position:fixed` and scrollWidth exceeds clientWidth by 148–151px at every tested viewport. At 1920x1080: scrollWidth=2039, clientWidth=1888.
- IMPACT: Fixed-position background element extends beyond viewport; no visible scrollbar but contributes to layout overflow calculations.

### approvals-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has `overflow:hidden` and scrollHeight=1547 vs clientHeight=676, clipping 871px of vertical content. Horizontal overflow grows from +139px at 1920x1080 to +255px at 1280x800 to +416px at 1024x768.
- IMPACT: The main page panel silently clips content. At narrower viewports the clipping worsens significantly, potentially hiding information if more content were rendered.

### approvals-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Both "Refresh" (header) and "Start Jarvis" (header) buttons use `type="submit"` instead of `type="button"`. They are not inside a `<form>` element.
- IMPACT: Semantically incorrect button type. If these buttons were ever placed inside a form, they would trigger unintended form submission.

### approvals-04
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Clicking "Refresh" produces no visible change, no console output, and no network requests. Same for "Start Jarvis". Both silently no-op in demo mode.
- IMPACT: Header action buttons give no feedback to the user — no loading state, no toast, no disabled state. User cannot tell whether the click registered.

## Summary
- Gate detected: no
- Total interactive elements: 2
- Elements clicked: 2
- P0: 0
- P1: 1
- P2: 3
