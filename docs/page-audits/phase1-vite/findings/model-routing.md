# Audit: Model Routing
URL: http://localhost:1420/model-routing
Audited at: 2026-04-09T22:23:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
1. `Error: desktop runtime unavailable` at `src/api/backend.ts:17:11` via `routerGetAccuracy` at `src/api/backend.ts:2424:10` called from `src/pages/ModelRouting.tsx:48:18` (commitHookEffectListMount)
2. `Error: desktop runtime unavailable` at `src/api/backend.ts:17:11` via `routerGetAccuracy` at `src/api/backend.ts:2424:10` called from `src/pages/ModelRouting.tsx:48:18` (React StrictMode double-invoke)

### Warnings
none

### Logs
none

### Info
none

### Debug
none

## Overflow

### 1920x1080
(actual viewport: 1888x895)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2035 clientWidth=1888 [OVERFLOW +147px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1944 clientWidth=1577 [OVERFLOW +367px]

### 1280x800
(actual viewport: 1248x615)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1344 clientWidth=1248 [OVERFLOW +96px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1109 clientWidth=937 [OVERFLOW +172px]

### 1024x768
(actual viewport: 992x583)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1067 clientWidth=992 [OVERFLOW +75px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=888 clientWidth=681 [OVERFLOW +207px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no type attr) | — | yes |
| 2 | Start Jarvis | button (no type attr) | — | yes |
| 3 | *(placeholder: "Enter task text to estimate difficulty...")* | input | — | yes |
| 4 | Estimate | button[type=button] | — | yes |

Note: Elements 1–2 are in the page banner/header bar, outside sidebar but also outside `<main>`. Elements 3–4 are inside `<main>`.

## Click sequence
### Click 1: "Refresh"
- Pathname before: /model-routing
- New console: clean
- Network failures: none
- Visible change: none — button is silently inert in demo mode
- Pathname after: /model-routing
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /model-routing
- New console: clean
- Network failures: none
- Visible change: none — button is silently inert in demo mode
- Pathname after: /model-routing
- Reverted: n/a

### Click 3: text input (focus)
- Pathname before: /model-routing
- New console: clean
- Network failures: none
- Visible change: input receives focus (cursor appears)
- Pathname after: /model-routing
- Reverted: n/a

### Click 4: "Estimate" (with empty input)
- Pathname before: /model-routing
- New console: clean
- Network failures: none
- Visible change: none — no validation message, no error, silently accepted empty input
- Pathname after: /model-routing
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 4
### Elements clicked: 4

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selector: `main input` — the task difficulty text input has placeholder but no `<label>`, `aria-label`, or `aria-labelledby`)
- Buttons without accessible name: 0

## Findings

### model-routing-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: Two `Error: desktop runtime unavailable` thrown on page load from `routerGetAccuracy()` at `src/api/backend.ts:2424` called in `useEffect` at `src/pages/ModelRouting.tsx:48`. Error is unhandled — propagates to React error boundary path.
- IMPACT: Routing accuracy data fails to load silently; user sees empty "No routing decisions" state with no explanation that an error occurred.

### model-routing-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` exceeds viewport at all three breakpoints: 1920 (+147px), 1280 (+96px), 1024 (+75px). Element is a decorative background layer.
- IMPACT: Potential horizontal scrollbar if parent overflow is not hidden; cosmetic layout bleed.

### model-routing-03
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel` horizontal overflow at all breakpoints: 1920 (+367px), 1280 (+172px), 1024 (+207px). Content panel is wider than its container.
- IMPACT: Page content extends beyond visible area; users cannot see or interact with clipped content without horizontal scrolling.

### model-routing-04
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel` vertical clip: scrollHeight=1516 clientHeight=637, overflow=hidden. 879px of content is invisible and inaccessible — no scrollbar rendered due to `overflow:hidden`.
- IMPACT: Lower portions of the Model Registry section may be clipped and permanently inaccessible to users.

### model-routing-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Duplicate H1 elements rendered simultaneously: "Model Routing" (in banner header, outside `<main>`) and "Predictive Model Routing" (inside `<main>`). Both are visible.
- IMPACT: Screen readers announce two H1 headings, violating single-H1-per-page best practice; confusing document outline.

### model-routing-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The task difficulty `<input>` has `placeholder="Enter task text to estimate difficulty..."` but no associated `<label>`, `aria-label`, or `aria-labelledby` attribute.
- IMPACT: Screen readers cannot announce the input's purpose; WCAG 1.3.1 and 4.1.2 violation.

### model-routing-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" banner buttons are missing the `type` attribute. Both default to `type="submit"` per HTML spec.
- IMPACT: Buttons may trigger unintended form submission if placed inside a `<form>`. Missing explicit type is a best-practice violation.

### model-routing-08
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Refresh" button click produces no console output, no network request, no visible change. Silently inert in demo mode.
- IMPACT: User clicks a prominently visible button and receives zero feedback — confusing UX.

### model-routing-09
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button click produces no console output, no network request, no visible change. Silently inert in demo mode.
- IMPACT: User clicks a prominently visible button and receives zero feedback.

### model-routing-10
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Estimate" button accepts empty input without validation — click produces no console output, no error message, no visible change. No `required` attribute on the input.
- IMPACT: User can submit an empty task estimation with no feedback; no client-side validation guards the form.

### model-routing-11
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Empty-state messages ("No routing decisions recorded yet…", "No models registered…") and the "Desktop runtime required" status banner lack `role="alert"` or `aria-live` attributes. None of the four `<section>` regions have `aria-live`.
- IMPACT: Screen readers will not announce status changes or empty-state messages; users relying on assistive technology miss dynamic content updates.

## Summary
- Gate detected: no
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 3
- P2: 8
