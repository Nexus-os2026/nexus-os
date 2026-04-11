# Audit: Voice
URL: http://localhost:1420/voice
Audited at: 2026-04-09T20:56:00+01:00
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
(actual viewport: 1888x895)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `.nexus-main-column`: scrollWidth=1630 clientWidth=1630 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2036 clientWidth=1888 (+148px) [OVERFLOW]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1715 clientWidth=1577 (+138px) [OVERFLOW]
  - `div` (unnamed child): scrollWidth=228 clientWidth=180 (+48px) [OVERFLOW]

### 1280x800
(actual viewport: 1248x615)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `.nexus-main-column`: scrollWidth=990 clientWidth=990 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1345 clientWidth=1248 (+97px) [OVERFLOW]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1199 clientWidth=937 (+262px) [OVERFLOW]
  - `div` (unnamed child): scrollWidth=228 clientWidth=180 (+48px) [OVERFLOW]

### 1024x768
(actual viewport: 992x583)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `.nexus-main-column`: scrollWidth=734 clientWidth=734 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1068 clientWidth=992 (+76px) [OVERFLOW]
  - `div` (unnamed child): scrollWidth=228 clientWidth=180 (+48px) [OVERFLOW]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | yes |
| 2 | Start Jarvis | button | — | yes |
| 3 | Clear | button | — | yes |
| 4 | Click to Talk | button | — | yes |
| 5 | Prefer Whisper (checkbox) | input[checkbox] | — | yes |
| 6 | Wake Word (text input) | input[text] | — | yes |
| 7 | Sample Rate | select | — | yes |
| 8 | On startup (checkbox) | input[checkbox] | — | yes |
| 9 | Load Whisper Model | button | — | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /voice
- New console: clean
- Network failures: none
- Visible change: none observed
- Pathname after: /voice
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /voice
- New console: clean
- Network failures: none
- Visible change: none observed — button silent in demo mode
- Pathname after: /voice
- Reverted: n/a

### Click 3: "Click to Talk"
- Pathname before: /voice
- New console: clean
- Network failures: none
- Visible change: button text changed to "Stop Listening"; status area shows "LISTENING..." and log entry "Voice assistant initialized. Checking backend availability..."
- Pathname after: /voice
- Reverted: n/a

### Click 4: "Prefer Whisper" checkbox
- Pathname before: /voice
- New console: clean
- Network failures: none
- Visible change: checkbox toggled from unchecked to checked
- Pathname after: /voice
- Reverted: n/a

### Click 5: "Wake Word" input
- Pathname before: /voice
- New console: clean
- Network failures: none
- Visible change: input did NOT receive focus on click (document.activeElement !== input)
- Pathname after: /voice
- Reverted: n/a

### Click 6: "Sample Rate" select
- Pathname before: /voice
- New console: clean
- Network failures: none
- Visible change: dropdown interaction (native select)
- Pathname after: /voice
- Reverted: n/a

### Click 7: "On startup" checkbox
- Pathname before: /voice
- New console: clean
- Network failures: none
- Visible change: checkbox toggled from unchecked to checked
- Pathname after: /voice
- Reverted: n/a

### Click 8: "Load Whisper Model"
- Pathname before: /voice
- New console: clean
- Network failures: none
- Visible change: log entries appeared — "Loading Whisper model..." followed by "Failed to load Whisper model: Error: desktop runtime unavailable"
- Pathname after: /voice
- Reverted: n/a

### Skipped (destructive)
- "Clear" — reason: destructive keyword "clear"

### Total interactive elements found: 9
### Elements clicked: 8 (of 9; 1 skipped as destructive)

## Accessibility
- Images without alt: 0
- Inputs without label: 2 (selectors: `input[type="text"]` (Wake Word), `select` (Sample Rate))
- Buttons without accessible name: 0
- Additional: all 5 buttons lack `type` attribute (default to `submit` outside a `<form>`)
- Additional: 2 labels exist (wrapping checkboxes) but the Wake Word input and Sample Rate select have no associated `<label>`, no `aria-label`, and no `id`
- Additional: all 4 inputs and 1 select have no `id` or `name` attributes
- Additional: no `<form>` element wraps any controls (formCount=0)

## Findings

### voice-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows at all viewports: +148px at 1920, +97px at 1280, +76px at 1024. This is a decorative background layer and is clipped by parent `overflow:hidden`, so no user-visible scrollbar appears.
- IMPACT: Cosmetic; no user-visible effect due to parent clipping, but contributes to inconsistent layout measurements.

### voice-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920|1280
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` overflows by +138px at 1920 and +262px at 1280. Root cause: child `.holo-panel__refraction` is absolutely positioned with `left: -315px` and `width: 2208px`, intentionally oversized for visual effect. Parent has `overflow: hidden`.
- IMPACT: Cosmetic; clipped by parent. No user-visible scrollbar. Same root cause as other page audits.

### voice-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Wake Word `input[type="text"]` and Sample Rate `select` have no `<label>`, no `aria-label`, no `id`, and no `name` attribute. Screen readers cannot identify these controls.
- IMPACT: Screen reader users cannot determine the purpose of these form controls.

### voice-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 5 buttons (`Refresh`, `Start Jarvis`, `Clear`, `Click to Talk/Stop Listening`, `Load Whisper Model`) lack a `type` attribute. Without `type`, buttons default to `type="submit"`. No `<form>` element exists on the page, so this is functionally harmless but semantically incorrect.
- IMPACT: Minor semantic issue; no functional bug since no `<form>` is present.

### voice-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 4 `<input>` elements and 1 `<select>` have no `id` or `name` attributes. No `<form>` element wraps the controls. Controls cannot be submitted as a form and are not programmatically identifiable.
- IMPACT: Form controls are not identifiable by assistive technology or standard form submission.

### voice-06
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Clicking the Wake Word `input[type="text"]` via JavaScript `.click()` does not move focus to the input (`document.activeElement !== input`). Programmatic `.focus()` does work. This suggests a click handler or overlay may be intercepting the click event.
- IMPACT: Users relying on click-to-focus (standard browser behavior) may be unable to edit the wake word field without tabbing into it.

### voice-07
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Load Whisper Model" button renders a visible error in the UI: "Failed to load Whisper model: Error: desktop runtime unavailable". This is an unformatted error string displayed directly in the voice log area.
- IMPACT: Raw error string exposed to users in demo mode. Should either be suppressed in demo mode or displayed as a user-friendly message.

### voice-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons produce no visible change, no console output, and no network requests when clicked. They are completely silent in demo mode.
- IMPACT: Buttons appear functional but provide no feedback. Users cannot tell if the click registered.

## Summary
- Gate detected: no
- Total interactive elements: 9
- Elements clicked: 8
- P0: 0
- P1: 2
- P2: 6
