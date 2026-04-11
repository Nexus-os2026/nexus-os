# Audit: Admin Policies
URL: http://localhost:1420/admin-policies
Audited at: 2026-04-10T00:44:42Z
Gate detected: false
Gate type: none

## Console (captured at native ~992x639, ALL messages)
### Errors
none

### Warnings
none

### Logs
none

### Info
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js?v=5144749d:21550:24 (x3 triplicated)

### Debug
- `[vite] connecting...` — @vite/client:494:8 (x3 triplicated)
- `[vite] connected.` — @vite/client:617:14 (x3 triplicated)

## Overflow

Note: `window.resizeTo()` and `setDeviceMetricsOverride` are non-functional in the Chrome extension context (confirmed in prior audits). Measurements taken at native viewport only (~992x639). Puppeteer screenshots at target viewports will supplement.

### 1920x1080
- Measured at native ~992x639 (resize unavailable)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main.nexus-shell-content.px-4.py-4`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1071 clientWidth=992 [OVERFLOW +79px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=741 clientWidth=681 [OVERFLOW +60px]
  - `span.nexus-sidebar-item-text`: scrollWidth=157 clientWidth=153 [OVERFLOW +4px] (x3, sidebar)

### 1280x800
- (resize unavailable — see note above)

### 1024x768
- (resize unavailable — see note above)

## Interactive elements (main content only)

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no type) | — | yes |
| 2 | Start Jarvis | button (no type) | — | yes |
| 3 | Edit Policy | button (no type) | — | yes |
| 4 | Templates | button (no type) | — | yes |
| 5 | History | button (no type) | — | yes |
| 6 | Scope (select) | combobox | — | yes |
| 7 | Max Autonomy Level (select) | combobox | — | yes |
| 8 | Fuel Limit / Agent | input[type=number] | — | yes |
| 9 | Fuel Limit / Workspace | input[type=number] | — | yes |
| 10 | HITL Required Above Tier (select) | combobox | — | yes |
| 11 | Allowed Providers | input[type=text] | — | yes |
| 12 | Self-Modify | input[type=checkbox] | — | yes (checked) |
| 13 | Internet | input[type=checkbox] | — | yes (checked) |
| 14 | PII Redaction | input[type=checkbox] | — | yes (checked) |
| 15 | Save Policy | button (no type) | — | yes |

## Click sequence

### Click 1: "Refresh"
- Pathname before: /admin-policies
- New console: clean
- Network failures: none
- Visible change: none — button is silently inert
- Pathname after: /admin-policies
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /admin-policies
- New console: clean
- Network failures: none
- Visible change: none — button is silently inert
- Pathname after: /admin-policies
- Reverted: n/a

### Click 3: "Edit Policy"
- Pathname before: /admin-policies
- New console: clean
- Network failures: none
- Visible change: none (Edit Policy tab was already active, form already displayed)
- Pathname after: /admin-policies
- Reverted: n/a

### Click 4: "Templates"
- Pathname before: /admin-policies
- New console: clean
- Network failures: none
- Visible change: tab switched (React batched — final state observed after click 5)
- Pathname after: /admin-policies
- Reverted: n/a

### Click 5: "History"
- Pathname before: /admin-policies
- New console: clean
- Network failures: none
- Visible change: main content replaced with History table — columns: Timestamp, User, Scope, Field, Old Value, New Value; body shows "No policy changes recorded"
- Pathname after: /admin-policies
- Reverted: n/a

### Click 6: "Scope select (change to workspace:default)"
- Pathname before: /admin-policies
- New console: clean
- Network failures: none
- Visible change: value changed to "workspace:default" in DOM (form was still in DOM during synchronous JS execution before React re-render)
- Pathname after: /admin-policies
- Reverted: n/a

### Click 7: "Max Autonomy Level (change to L5 Full)"
- Pathname before: /admin-policies
- New console: clean
- Network failures: none
- Visible change: value changed to "5" in DOM (sync execution)
- Pathname after: /admin-policies
- Reverted: n/a

### Click 8: "Fuel Limit / Agent input (focus)"
- Pathname before: /admin-policies
- New console: clean
- Network failures: none
- Visible change: focused (sync execution, elements still in DOM)
- Pathname after: /admin-policies
- Reverted: n/a

### Click 9: "Fuel Limit / Workspace input (focus)"
- Pathname before: /admin-policies
- New console: clean
- Network failures: none
- Visible change: focused (sync execution)
- Pathname after: /admin-policies
- Reverted: n/a

### Click 10: "HITL Required Above Tier (change to Tier 2)"
- Pathname before: /admin-policies
- New console: clean
- Network failures: none
- Visible change: value changed to "2" in DOM (sync execution)
- Pathname after: /admin-policies
- Reverted: n/a

### Skipped (destructive)
- none

### Total interactive elements found: 15
### Elements clicked: 10 (capped at 10)
### Not clicked: Allowed Providers (input), Self-Modify (checkbox), Internet (checkbox), PII Redaction (checkbox), Save Policy (button)

## Accessibility
- Images without alt: 0
- Inputs without label: 6 (selectors: `main select:nth-of-type(1)` [Scope], `main select:nth-of-type(2)` [Max Autonomy Level], `main select:nth-of-type(3)` [HITL Required Above Tier], `main input[type="number"]:nth-of-type(1)` [Fuel Limit / Agent], `main input[type="number"]:nth-of-type(2)` [Fuel Limit / Workspace], `main input[type="text"]` [Allowed Providers]) — all have visual `<label>` siblings but no `for`/`id` association, no `aria-label`, not wrapped in `<label>`
- Buttons without accessible name: 0

## Findings

### admin-policies-01
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 6 form inputs in `main > section.admin-shell` have visual labels (`<label>` elements with text "Scope:", "Max Autonomy Level", "Fuel Limit / Agent", "Fuel Limit / Workspace", "HITL Required Above Tier", "Allowed Providers") but zero programmatic association. All `<select>` and `<input>` elements lack `id` attributes; corresponding `<label>` elements lack `for` attributes. Inputs are not wrapped inside their labels. No `aria-label` or `aria-labelledby` present. Screen readers will announce comboboxes by their selected option text (e.g., "Global") instead of by their purpose (e.g., "Scope").
- IMPACT: Screen reader users cannot determine what each form field controls, making the policy editor unusable for assistive technology users.

### admin-policies-02
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page: "Admin Policy" in `<header>` (banner landmark) and "Policy Editor" in `<main>`. `document.querySelectorAll('h1').length === 2`.
- IMPACT: Violates WCAG single-H1 best practice; screen reader users cannot determine the primary page heading.

### admin-policies-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 6 buttons lack explicit `type` attribute: "Edit Policy", "Templates", "History", "Save Policy" (in main), "Refresh", "Start Jarvis" (in banner/header). Without `type="button"`, these default to `type="submit"` per HTML spec, which can cause unintended form submission if placed inside a `<form>`.
- IMPACT: Potential for unintended form submission behavior if markup changes.

### admin-policies-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Edit Policy", "Templates", and "History" buttons function as tab-like navigation (switching between form view, templates view, and history view) but lack ARIA tab pattern attributes. No `role="tablist"` on container, no `role="tab"` on buttons, no `aria-selected` on active tab, no `role="tabpanel"` on content panels.
- IMPACT: Screen reader users cannot perceive the tab navigation pattern or determine which tab is active.

### admin-policies-05
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all (measured at native ~992x639)
- EVIDENCE: `div.living-background` scrollWidth=1071 > clientWidth=992 (79px horizontal overflow). `section.holo-panel.holo-panel-mid` scrollWidth=741 > clientWidth=681 (60px overflow).
- IMPACT: Background element and holo-panel extend beyond viewport causing potential horizontal scrollbar or clipped content.

### admin-policies-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (in header) clicked — no console output, no network request, no visible change. "Start Jarvis" button (in header) clicked — same silent no-op behavior. Both buttons produce zero feedback in demo mode.
- IMPACT: Users receive no indication that an action was attempted or that the feature requires the Tauri backend.

### admin-policies-07
- SEVERITY: P2
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: Vite HMR messages triplicated on page load: `[vite] connecting...` (x3) at @vite/client:494:8, `[vite] connected.` (x3) at @vite/client:617:14, React DevTools info message (x3) at chunk-NUMECXU6.js:21550:24. Total 9 messages instead of expected 3.
- IMPACT: Suggests triple mounting or multiple Vite client instances; may indicate React StrictMode double-render plus an extra mount cycle.

### admin-policies-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<header>` element containing "Admin Policy" H1, "Refresh" button, and "Start Jarvis" button does not have explicit `role="banner"`. Chrome computes implicit banner role, but the heading and action buttons within it are outside the `<main>` landmark.
- IMPACT: Minor — implicit semantics work in most screen readers, but explicit role improves robustness.

## Summary
- Gate detected: no
- Total interactive elements: 15
- Elements clicked: 10
- P0: 0
- P1: 1
- P2: 7
