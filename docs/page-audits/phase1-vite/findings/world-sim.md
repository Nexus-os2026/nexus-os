# Audit: World Sim
URL: http://localhost:1420/world-sim
Audited at: 2026-04-09T22:50:00Z
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

### 1920x1080
_(resize_window does not change viewport in Tauri-wrapped Chrome; actual viewport locked at 1888x951)_
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]

### 1280x800
_(resize did not take effect — viewport remained 1888x951; measurements identical to above)_
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]

### 1024x768
_(resize did not take effect — viewport remained 1888x951; measurements identical to above)_
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Dismiss | button (type=submit) | — | yes |
| 2 | (Scenario description...) | input[text] | — | yes |
| 3 | (Actions JSON array) | textarea | — | yes |
| 4 | Submit & Run | button (type=submit) | — | no (disabled) |
| 5 | (unlabeled agent selector) | select | — | yes |

_Header bar (outside main): "Refresh" button, "Start Jarvis" button — not inside `main.nexus-shell-content`._

## Click sequence
### Click 1: "Dismiss"
- Pathname before: /world-sim
- New console: clean
- Network failures: none
- Visible change: Error alert "Error: desktop runtime unavailable" was dismissed and removed from DOM
- Pathname after: /world-sim
- Reverted: n/a

### Click 2: Scenario description input
- Pathname before: /world-sim
- New console: clean
- Network failures: none
- Visible change: Input received focus (cursor appeared in text field)
- Pathname after: /world-sim
- Reverted: n/a

### Click 3: Actions JSON textarea
- Pathname before: /world-sim
- New console: clean
- Network failures: none
- Visible change: Textarea received focus
- Pathname after: /world-sim
- Reverted: n/a

### Click 4: "Submit & Run" (disabled)
- Pathname before: /world-sim
- New console: clean
- Network failures: none
- Visible change: None (button is disabled)
- Pathname after: /world-sim
- Reverted: n/a

### Click 5: Agent selector (select)
- Pathname before: /world-sim
- New console: clean
- Network failures: none
- Visible change: Select opened but showed empty dropdown (0 options)
- Pathname after: /world-sim
- Reverted: n/a

### Click 6: "Refresh" (header)
- Pathname before: /world-sim
- New console: clean
- Network failures: none
- Visible change: None — completely inert in demo mode
- Pathname after: /world-sim
- Reverted: n/a

### Click 7: "Start Jarvis" (header)
- Pathname before: /world-sim
- New console: clean
- Network failures: none
- Visible change: None — completely inert in demo mode
- Pathname after: /world-sim
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 7 (5 in main + 2 in header)
### Elements clicked: 7

## Accessibility
- Images without alt: 0
- Inputs without label: 3 (selectors: `input[type=text][placeholder="Scenario description..."]`, `textarea[placeholder="Actions JSON array"]`, `select[select-one]`)
- Buttons without accessible name: 0
- Sections without aria-label: 1 (the main `section.holo-panel` region)

## Findings

### world-sim-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 1920 (all measured at 1888x951)
- EVIDENCE: `div.living-background` scrollWidth=2039 > clientWidth=1888 (+151px horizontal overflow)
- IMPACT: Background element overflows viewport, may cause horizontal scrollbar on non-Tauri browsers

### world-sim-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 1920 (all measured at 1888x951)
- EVIDENCE: `section.holo-panel.holo-panel-mid` has overflow:hidden with scrollWidth=2172 clientWidth=1577 (595px clipped horizontally) and scrollHeight=1396 clientHeight=637 (759px clipped vertically)
- IMPACT: holo-panel silently clips significant content in both axes; users cannot scroll to see clipped form fields, history, or risk guide

### world-sim-03
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two H1 elements on page: "World Simulation" (in shell header `banner`) and "World Simulation Engine" (in `main section`). Only one H1 should exist per page.
- IMPACT: Screen readers announce two top-level headings, breaking document outline and navigation

### world-sim-04
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Agent selector `select` element renders with 0 `<option>` elements in demo mode. Clicking opens an empty dropdown.
- IMPACT: Users cannot select an agent to run simulations; form is non-functional without agent selection

### world-sim-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 3 form controls lack label association: `input[type=text]` (placeholder="Scenario description..."), `textarea` (placeholder="Actions JSON array"), `select` (agent selector). None have `<label for>`, `aria-label`, or `aria-labelledby`.
- IMPACT: Screen readers cannot identify form field purposes; relies solely on placeholder text which disappears on input

### world-sim-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Submit & Run" button has `type="submit"` but is permanently disabled in demo mode with no visible explanation. The "Dismiss" button also has `type="submit"` instead of `type="button"`.
- IMPACT: Buttons with type="submit" may trigger unintended form submission if placed inside a `<form>` element; disabled state has no tooltip or explanation

### world-sim-07
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Refresh" button (header) click produces no console output, no navigation, and no visible change in demo mode
- IMPACT: Button appears interactive but is completely inert with no user feedback

### world-sim-08
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button (header) click produces no console output, no navigation, and no visible change in demo mode
- IMPACT: Button appears interactive but is completely inert with no user feedback

### world-sim-09
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `section.holo-panel` region element lacks `aria-label` or `aria-labelledby` attribute
- IMPACT: Screen readers announce a generic unnamed region landmark

### world-sim-10
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons are rendered in the shell header `banner`, outside `main.nexus-shell-content`. They are page-specific controls that should be within the main content landmark.
- IMPACT: Keyboard users navigating by landmark will not find these controls in the expected main region

## Summary
- Gate detected: no
- Total interactive elements: 7 (5 main + 2 header)
- Elements clicked: 7
- P0: 0
- P1: 4
- P2: 6
