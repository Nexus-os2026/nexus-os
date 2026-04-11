# Audit: Scheduler
URL: http://localhost:1420/scheduler
Audited at: 2026-04-09T19:32:00+01:00
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
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js:21550:24

### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080
(effective viewport 1888x895)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1629 clientWidth=1577 [OVERFLOW +52px]

### 1280x800
(effective viewport 1248x615)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1110 clientWidth=937 [OVERFLOW +173px]

### 1024x768
(effective viewport 992x583)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1071 clientWidth=992 [OVERFLOW +79px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=874 clientWidth=681 [OVERFLOW +193px]

## Interactive elements (main content only)

Initial state (before form open):

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (header) | — | yes |
| 2 | Start Jarvis | button (header) | — | yes |
| 3 | Refresh | button (main region) | — | yes |
| 4 | + New Schedule | button | — | yes |
| 5 | × | button (error dismiss) | — | yes |

After "+ New Schedule" click, form appears with additional elements:

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 6 | Name | input (placeholder: my-scheduled-task) | — | yes |
| 7 | Agent DID | input (placeholder: agent-uuid) | — | yes |
| 8 | Trigger Type | select (Cron/Interval/Webhook/Event/One-Shot) | — | yes |
| 9 | Cron Expression | select (7 presets) | — | yes |
| 10 | Task Type | select (Run Agent/Send Notification/Execute Command) | — | yes |
| 11 | Max Fuel Per Run | input[number] | — | yes |
| 12 | Task Parameters (JSON) | textarea | — | yes |
| 13 | Priority | select (low/normal/high/critical) | — | yes |
| 14 | Enabled | input[checkbox] | — | yes |
| 15 | Cancel | button | — | yes |
| 16 | Create Schedule | button | — | yes |

## Click sequence

### Click 1: "Refresh" (header)
- Pathname before: /scheduler
- New console: clean
- Network failures: none
- Visible change: none observed
- Pathname after: /scheduler
- Reverted: n/a

### Click 2: "Start Jarvis" (header)
- Pathname before: /scheduler
- New console: clean
- Network failures: none
- Visible change: none observed — button is a dead click in demo mode
- Pathname after: /scheduler
- Reverted: n/a

### Click 3: "Refresh" (main region)
- Pathname before: /scheduler
- New console: clean
- Network failures: none
- Visible change: none observed
- Pathname after: /scheduler
- Reverted: n/a

### Click 4: "+ New Schedule"
- Pathname before: /scheduler
- New console: clean
- Network failures: none
- Visible change: "Create Schedule" form appears with Name, Agent DID, Trigger Type, Cron Expression, Task Type, Max Fuel Per Run, Task Parameters (JSON), Priority, Enabled fields. "Cancel" and "Create Schedule" buttons replace "+ New Schedule". "+ New Schedule" button changes to "Cancel".
- Pathname after: /scheduler
- Reverted: n/a

### Click 5: "×" (error bar dismiss)
- Pathname before: /scheduler
- New console: clean
- Network failures: none
- Visible change: error bar "Error: desktop runtime unavailable" dismissed
- Pathname after: /scheduler
- Reverted: n/a

### Click 6: "Create Schedule" (form submit)
- Pathname before: /scheduler
- New console: clean
- Network failures: none
- Visible change: error bar "Error: desktop runtime unavailable" re-appears with × dismiss button. Form remains visible.
- Pathname after: /scheduler
- Reverted: n/a

### Click 7: "Cancel" (form)
- Pathname before: /scheduler
- New console: clean
- Network failures: none
- Visible change: form closes, returns to empty state "No schedules yet". Error bar persists.
- Pathname after: /scheduler
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 5 (initial state), 16 (with form open)
### Elements clicked: 7

## Accessibility
- Images without alt: 0
- Inputs without label: 0 (all form inputs wrapped in `<label>` elements)
- Buttons without accessible name: 0

## Findings

### scheduler-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows at all viewports: +151px at 1920x1080, +100px at 1280x800, +79px at 1024x768. Element has `position: fixed` and `overflow: hidden` but `width: 1888px` exceeds viewport at smaller sizes. Overflow is hidden from user scroll but element dimensions exceed clientWidth.
- IMPACT: Decorative background layer exceeds viewport bounds; hidden by position:fixed but contributes to layout measurement anomalies.

### scheduler-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has `overflow: hidden` but content overflows at all viewports: +52px at 1920x1080, +173px at 1280x800, +193px at 1024x768. At 1024x768 the panel clips 193px of content silently.
- IMPACT: Content inside the main page panel is silently clipped by `overflow: hidden`, potentially hiding form fields or interactive elements at narrower viewports.

### scheduler-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: All 5 main-content buttons lack an explicit `type` attribute (none have `type="button"` or `type="submit"`). Buttons are: "Refresh" (header), "Start Jarvis", "Refresh" (main), "+ New Schedule", "×". No `<form>` element wraps the Create Schedule form, so implicit `type="submit"` has no form to submit — but this is semantically incorrect.
- IMPACT: Without explicit `type="button"`, browsers default to `type="submit"`. While there is no `<form>` element present, the missing type attribute is a semantic and best-practice issue.

### scheduler-04
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: The "Create Schedule" form has no `<form>` element wrapping it. Inputs (Name, Agent DID, Trigger Type, etc.) and the "Create Schedule" submit button are rendered as siblings inside a `<div>`, not within a `<form>`. There is no `<form>` element on the page (`document.querySelectorAll('form').length === 0`).
- IMPACT: No native form semantics — Enter key does not submit, browser autofill may not work, screen readers cannot announce form boundaries. Assistive technology users cannot navigate to the form as a landmark.

### scheduler-05
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: Clicking "Create Schedule" with empty required fields (Name and Agent DID are blank) shows "Error: desktop runtime unavailable" instead of client-side validation. No field-level validation messages appear. The error message is identical to the page-load backend-unavailable error, making it impossible to distinguish a validation failure from a backend connectivity failure.
- IMPACT: Users cannot tell whether the schedule creation failed due to missing fields or due to the backend being offline. In production (with backend), submitting empty fields may succeed or produce confusing server errors.

### scheduler-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: Header "Refresh" button and main-region "Refresh" button both produce no visible feedback on click — no loading spinner, no flash, no console output. "Start Jarvis" button also produces zero response. All three are dead buttons in demo mode with no user feedback.
- IMPACT: Users clicking these buttons receive no indication that the action was attempted or that the feature requires the desktop runtime.

## Summary
- Gate detected: no
- Total interactive elements: 16 (5 initial + 11 in form)
- Elements clicked: 7
- P0: 0
- P1: 1
- P2: 5
