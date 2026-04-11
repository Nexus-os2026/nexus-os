# Audit: Perception
URL: http://localhost:1420/perception
Audited at: 2026-04-09T22:55:00Z
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
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js:21550

### Debug
- `[vite] connecting...` — @vite/client:494
- `[vite] connected.` — @vite/client:617

## Overflow

### 1920x1080
*(viewport locked at 1888x951 — Tauri-wrapped Chrome ignores resize_window; measurements taken at native viewport)*
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px] (fixed-position background, overflow:hidden — cosmetic only, not user-visible)
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px] (sidebar text truncation)

### 1280x800
*(resize_window had no effect — viewport remained at 1888x951; measurements identical to above)*
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as 1920x1080

### 1024x768
*(resize_window had no effect — viewport remained at 1888x951; measurements identical to above)*
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as 1920x1080

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (type="submit") | — | yes |
| 2 | Start Jarvis | button (type="submit") | — | yes |
| 3 | Groq (llama-4-scout) | select | — | yes |
| 4 | API Key | input (type="password") | — | yes |
| 5 | Model ID | input (type="text") | — | yes |
| 6 | Initialize | button (type="submit") | — | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /perception
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /perception
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /perception
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /perception
- Reverted: n/a

### Click 3: "Groq (llama-4-scout)" (select)
- Pathname before: /perception
- New console: clean
- Network failures: none
- Visible change: select dropdown opened, 2 options visible (Groq, NVIDIA NIM)
- Pathname after: /perception
- Reverted: n/a

### Click 4: "API Key" (input)
- Pathname before: /perception
- New console: clean
- Network failures: none
- Visible change: input focused, cursor appeared
- Pathname after: /perception
- Reverted: n/a

### Click 5: "Model ID" (input)
- Pathname before: /perception
- New console: clean
- Network failures: none
- Visible change: input focused, cursor appeared
- Pathname after: /perception
- Reverted: n/a

### Click 6: "Initialize" (button)
- Pathname before: /perception
- New console: clean
- Network failures: none
- Visible change: none — no validation error, no success feedback, no loading state, silent no-op with empty form fields
- Pathname after: /perception
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 6
### Elements clicked: 6

## Accessibility
- Images without alt: 0
- Inputs without label: 3 (selectors: `select` [no id, no aria-label, no associated label], `input[type="password"][placeholder="API Key"]` [no id, no aria-label], `input[type="text"][placeholder="Model ID"]` [no id, no aria-label])
- Buttons without accessible name: 0
- Duplicate H1: 2 (`h1` "Perception" in page header, `h1` "Multi-Modal Perception" in main content)
- Unlabeled sections: 1 (`section.holo-panel.holo-panel-mid` — no aria-label or aria-labelledby)

## Findings

### perception-01
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 3 form controls have no programmatic label association. `select` element has no id, no aria-label, no name attribute. `input[type="password"][placeholder="API Key"]` and `input[type="text"][placeholder="Model ID"]` rely solely on placeholder text for identification — no `<label>`, no `id`, no `aria-label`.
- IMPACT: Screen readers cannot identify form fields; placeholder text disappears on input, removing the only visual label.

### perception-02
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page: "Perception" in the page header bar and "Multi-Modal Perception" inside `<main>`. HTML spec recommends one `<h1>` per page.
- IMPACT: Confuses document outline for assistive technology; screen readers may misinterpret page structure.

### perception-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` has no `aria-label` or `aria-labelledby` attribute. This is the primary content region containing the vision provider initialization form.
- IMPACT: Screen readers announce an unnamed region, reducing navigability.

### perception-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` has scrollWidth=1716 vs clientWidth=1577 (overflow of 139px). The panel also has overflow:hidden with scrollHeight=1039 vs clientHeight=693, silently clipping 346px of vertical content.
- IMPACT: Content below the visible area of the holo-panel is invisible and unreachable by scroll; users may miss form elements or additional content if the page grows.

### perception-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (type="submit") in page header produces no visible change, no console output, and no network request when clicked in demo mode.
- IMPACT: Button appears functional but does nothing, misleading users.

### perception-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button (type="submit") in page header produces no visible change, no console output, and no network request when clicked in demo mode.
- IMPACT: Button appears functional but does nothing, misleading users.

### perception-07
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Initialize" button (type="submit") clicked with all form fields empty (API Key blank, Model ID blank) produces no validation error, no console output, no loading indicator, and no visible feedback of any kind. The form silently accepts an empty submission.
- IMPACT: Users receive zero feedback on form submission — no indication of success, failure, or validation requirements. In demo mode, should show a validation message or a mock success/failure state.

### perception-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 3 buttons on the page use `type="submit"` — "Refresh", "Start Jarvis", and "Initialize". The Refresh and Start Jarvis buttons are not inside a `<form>` element but still have submit type. Only "Initialize" is contextually a submit action.
- IMPACT: Incorrect button type may cause unexpected form submission behavior if these buttons are later placed inside a form; semantically misleading.

## Summary
- Gate detected: no
- Total interactive elements: 6
- Elements clicked: 6
- P0: 0
- P1: 2
- P2: 6
