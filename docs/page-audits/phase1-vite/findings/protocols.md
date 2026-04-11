# Audit: Protocols
URL: http://localhost:1420/protocols
Audited at: 2026-04-09T20:45:00Z
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
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `.nexus-main-column`: scrollWidth=1630 clientWidth=1630 [OK]
- other overflowing:
  - `DIV.living-background`: scrollWidth=2032 clientWidth=1888 [OVERFLOW +144px]
  - `SECTION.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1903 clientWidth=1577 [OVERFLOW +326px, hidden by overflow:hidden]

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `.nexus-main-column`: scrollWidth=990 clientWidth=990 [OK]
- other overflowing:
  - `DIV.living-background`: scrollWidth=1341 clientWidth=1248 [OVERFLOW +93px]
  - `SECTION.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1007 clientWidth=937 [OVERFLOW +70px, hidden by overflow:hidden]

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `.nexus-main-column`: scrollWidth=734 clientWidth=734 [OK]
- other overflowing:
  - `DIV.living-background`: scrollWidth=1064 clientWidth=992 [OVERFLOW +72px]
  - `SECTION.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=944 clientWidth=681 [OVERFLOW +263px, hidden by overflow:hidden]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | yes |
| 2 | Start Jarvis | button | — | yes |
| 3 | (placeholder: Agent base URL) | input[text] | — | yes |
| 4 | Discover | button | — | no |
| 5 | (placeholder: Agent URL) | input[text] | — | yes |
| 6 | (placeholder: Task message...) | textarea | — | yes |
| 7 | Send Task | button | — | no |
| 8 | (placeholder: Agent URL) | input[text] | — | yes |
| 9 | (placeholder: Task ID) | input[text] | — | yes |
| 10 | Get Status | button | — | no |
| 11 | Cancel Task | button | — | no |
| 12 | Get Agent Card | button | — | yes |
| 13 | List Skills | button | — | yes |
| 14 | Get Status | button | — | yes |
| 15 | (placeholder: Server name) | input[text] | — | yes |
| 16 | (placeholder: URL e.g. http://localhost:8080) | input[text] | — | yes |
| 17 | HTTP / SSE / Stdio | select | — | yes |
| 18 | (placeholder: Auth token) | input[password] | — | yes |
| 19 | Add Server | button | — | no |
| 20 | (placeholder: Tool name) | input[text] | — | yes |
| 21 | (placeholder: Arguments JSON) | textarea | — | yes |
| 22 | Call Tool | button | — | no |
| 23 | (placeholder: Server ID) | input[text] | — | yes |
| 24 | (placeholder: Display name) | input[text] | — | yes |
| 25 | (placeholder: Command e.g. npx) | input[text] | — | yes |
| 26 | (placeholder: Args space-separated) | input[text] | — | yes |
| 27 | Register | button | — | yes |
| 28 | (placeholder: Server ID) | input[text] | — | yes |
| 29 | Discover | button | — | yes |
| 30 | (placeholder: Server ID) | input[text] | — | yes |
| 31 | (placeholder: Tool name) | input[text] | — | yes |
| 32 | (placeholder: {"key": "value"}) | textarea | — | yes |
| 33 | Execute | button | — | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /protocols
- New console: clean
- Network failures: none
- Visible change: none observed
- Pathname after: /protocols
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /protocols
- New console: clean
- Network failures: none
- Visible change: none observed
- Pathname after: /protocols
- Reverted: n/a

### Click 3: input "Agent base URL"
- Pathname before: /protocols
- New console: clean
- Network failures: none
- Visible change: none — click did not focus the input (document.activeElement !== el). Manual .focus() call does work.
- Pathname after: /protocols
- Reverted: n/a

### Click 4: "Discover" (disabled)
- Pathname before: /protocols
- New console: clean
- Network failures: none
- Visible change: none (button disabled)
- Pathname after: /protocols
- Reverted: n/a

### Click 5: input "Agent URL"
- Pathname before: /protocols
- New console: clean
- Network failures: none
- Visible change: none — click did not focus
- Pathname after: /protocols
- Reverted: n/a

### Click 6: textarea "Task message..."
- Pathname before: /protocols
- New console: clean
- Network failures: none
- Visible change: none — click did not focus
- Pathname after: /protocols
- Reverted: n/a

### Click 7: "Send Task" (disabled)
- Pathname before: /protocols
- New console: clean
- Network failures: none
- Visible change: none (button disabled)
- Pathname after: /protocols
- Reverted: n/a

### Click 8: input "Agent URL" (Task Status section)
- Pathname before: /protocols
- New console: clean
- Network failures: none
- Visible change: none — click did not focus
- Pathname after: /protocols
- Reverted: n/a

### Click 9: input "Task ID"
- Pathname before: /protocols
- New console: clean
- Network failures: none
- Visible change: none — click did not focus
- Pathname after: /protocols
- Reverted: n/a

### Click 10: "Get Status" (disabled, Task Status section)
- Pathname before: /protocols
- New console: clean
- Network failures: none
- Visible change: none (button disabled)
- Pathname after: /protocols
- Reverted: n/a

### Skipped (destructive)
- "Cancel Task" — reason: destructive keyword "cancel"

### Total interactive elements found: 33
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 19 (all inputs/textareas/selects on the page rely on placeholder only — zero `<label>` elements exist in main content, zero `aria-label` attributes)
  - selectors: `INPUT[placeholder="Agent base URL (e.g. http://localhost:9000)"]`, `INPUT[placeholder="Agent URL"]` (x2), `TEXTAREA[placeholder="Task message..."]`, `INPUT[placeholder="Task ID"]`, `INPUT[placeholder="Server name"]`, `INPUT[placeholder="URL (e.g. http://localhost:8080)"]`, `SELECT` (transport type), `INPUT[placeholder="Auth token (optional)"]`, `INPUT[placeholder="Tool name"]` (x2), `TEXTAREA[placeholder="Arguments JSON..."]`, `INPUT[placeholder="Server ID"]` (x3), `INPUT[placeholder="Display name"]`, `INPUT[placeholder="Command (e.g. npx)"]`, `INPUT[placeholder="Args (space-separated)"]`, `TEXTAREA[placeholder="{\"key\": \"value\"}"]`
- Buttons without accessible name: 0

## Findings

### protocols-01
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 19 form inputs (15 text inputs, 3 textareas, 1 select) have zero associated `<label>` elements and zero `aria-label` attributes. The page has 0 `<label>` elements total. Inputs rely solely on `placeholder` for identification.
- IMPACT: Screen readers cannot programmatically associate any input with its purpose; placeholder text disappears on input, leaving sighted low-vision users without context.

### protocols-02
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 14 `<button>` elements in main content have no explicit `type` attribute (`getAttribute('type')` returns null). The browser default for buttons is `type="submit"`. None of the buttons are inside a `<form>` element, so the submit type is semantically incorrect.
- IMPACT: Buttons default to submit behavior which is meaningless outside a form context; assistive technology may misreport button purpose.

### protocols-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `SECTION.holo-panel.holo-panel-mid.nexus-page-panel` has `overflow: hidden` and its child `.holo-panel__refraction` is 2208px wide vs container 1577px at 1920x1080. The overflow delta is +326px (1920), +70px (1280), +263px (1024). Content is silently clipped.
- IMPACT: The decorative refraction layer extends well beyond the panel boundary; while `overflow:hidden` prevents scrollbars, any future content placed in the overflow zone would be invisible.

### protocols-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<header>` elements exist in the document — `header.nexus-shell-header` (the app-level top bar) and `header.proto-header` (inside the protocols page content). Both implicitly create `role="banner"` landmarks, creating duplicate banner landmarks.
- IMPACT: Screen reader landmark navigation lists two "banner" regions with no distinguishing label, making it harder to orient within the page.

### protocols-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Programmatic `.click()` on text inputs and textareas does not move focus to the element (`document.activeElement !== el` after click). Tested on elements #3, #5, #6, #8, #9. Manual `.focus()` does work. This is consistent across all input elements.
- IMPACT: In demo mode, click-to-focus on inputs appears non-functional via programmatic click, though manual `.focus()` succeeds — may indicate an event handler (e.g. `stopPropagation` or overlay) intercepting click events before they reach the input.

## Summary
- Gate detected: no
- Total interactive elements: 33
- Elements clicked: 10
- P0: 0
- P1: 1
- P2: 4
