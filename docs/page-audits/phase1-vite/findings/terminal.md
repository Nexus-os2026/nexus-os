# Audit: Terminal
URL: http://localhost:1420/terminal
Audited at: 2026-04-09T19:43:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
1. `TypeError: Cannot read properties of undefined (reading 'transformCallback')` at `chunk-G7S6KQDI.js:22:37` via `@tauri-apps_api_event.js:38:14` via `NexusCodePage.tsx:315:15` — triggered by `listen()` call in `useEffect` at `NexusCodePage.tsx:574:5` (React commitHookEffectListMount)
2. `TypeError: Cannot read properties of undefined (reading 'transformCallback')` at `chunk-G7S6KQDI.js:22:37` via `@tauri-apps_api_event.js:38:14` via `NexusCodePage.tsx:315:15` — triggered by React StrictMode double-invoke at `NexusCodePage.tsx:574:5` (invokePassiveEffectMountInDEV)
### Warnings
1. `Removing borderBottomColor borderBottom` — React warning: setting a style property during rerender when a conflicting property is set. Shorthand `borderBottom` conflicts with `borderBottomColor`. Source: `NexusCodePage.tsx:277:35` inside tab div, triggered on Computer Use tab click.
### Logs
none
### Info
none
### Debug
none

## Overflow

### 1920x1080 (effective viewport 1888x895)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2036 clientWidth=1888 [OVERFLOW +148px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1907 clientWidth=1577 [OVERFLOW +330px]

### 1280x800 (effective viewport 1248x615)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1344 clientWidth=1248 [OVERFLOW +96px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1192 clientWidth=937 [OVERFLOW +255px]

### 1024x768 (effective viewport 992x583)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1068 clientWidth=992 [OVERFLOW +76px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=928 clientWidth=681 [OVERFLOW +247px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Chat | div (cursor:pointer) | — | yes |
| 2 | Computer Use | div (cursor:pointer) | — | yes |
| 3 | Ask the governed agent... | input[type=text] | — | yes |
| 4 | Send | button (type=null) | — | no (disabled until input) |
| 5 | Screenshot | button (Computer Use tab) | — | yes |
| 6 | Describe a task for the computer-use agent... | input[type=text] (Computer Use tab) | — | yes |
| 7 | Run | button (Computer Use tab) | — | no (disabled until input) |
| 8 | Refresh | button (header) | — | yes |
| 9 | Start Jarvis | button (header) | — | yes |

## Click sequence
### Click 1: "Chat" (tab div)
- Pathname before: /terminal
- New console: clean
- Network failures: none
- Visible change: Tab already active (blue border); no content change
- Pathname after: /terminal
- Reverted: n/a

### Click 2: "Computer Use" (tab div)
- Pathname before: /terminal
- New console: WARNING — "Removing borderBottomColor borderBottom" — React style conflict warning at NexusCodePage.tsx:277:35
- Network failures: none
- Visible change: Tab switches to purple border; content changes to show "Screenshot" button, new input "Describe a task...", and "Run" button; description text changes to "Computer Use Agent"
- Pathname after: /terminal
- Reverted: n/a

### Click 3: "Screenshot"
- Pathname before: /terminal
- New console: clean
- Network failures: none
- Visible change: Error message appears in chat area: "Screenshot failed: Error: desktop runtime unavailable"
- Pathname after: /terminal
- Reverted: n/a

### Click 4: "Chat" (tab div, switch back)
- Pathname before: /terminal
- New console: clean
- Network failures: none
- Visible change: Switched back to Chat tab; chat area shows prior messages including "Screenshot failed" and previous send attempt
- Pathname after: /terminal
- Reverted: n/a

### Click 5: Input "Ask the governed agent..." (focus + type)
- Pathname before: /terminal
- New console: clean
- Network failures: none
- Visible change: Input receives text "test command"; Send button becomes enabled
- Pathname after: /terminal
- Reverted: n/a

### Click 6: "Send"
- Pathname before: /terminal
- New console: clean
- Network failures: none
- Visible change: User message "test command" appears in chat area; response "Failed: Error: desktop runtime unavailable" appears; input clears
- Pathname after: /terminal
- Reverted: n/a

### Click 7: "Refresh" (header)
- Pathname before: /terminal
- New console: clean
- Network failures: none
- Visible change: No visible change — silent no-op in demo mode
- Pathname after: /terminal
- Reverted: n/a

### Click 8: "Start Jarvis" (header)
- Pathname before: /terminal
- New console: clean
- Network failures: none
- Visible change: No visible change — silent no-op in demo mode
- Pathname after: /terminal
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 9
### Elements clicked: 8 (under cap of 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `input[placeholder="Ask the governed agent..."]`)
- Buttons without accessible name: 0
- Tab divs without role: 2 — `div` with text "Chat" and `div` with text "Computer Use" use `cursor:pointer` but lack `role="tab"`, `tabindex`, and `aria-selected`
- Send button missing `type` attribute: `button` with label "Send" has `type=null` (should be `type="button"`)

## Findings

### terminal-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: Two `TypeError: Cannot read properties of undefined (reading 'transformCallback')` errors thrown at page mount from `NexusCodePage.tsx:315:15` calling `listen()` from `@tauri-apps_api_event.js:38:14`. Unguarded Tauri API call in `useEffect` at line 574.
- IMPACT: Unguarded Tauri event listener crashes on mount in browser/demo mode; pollutes console and may break event subscriptions needed for terminal functionality.

### terminal-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows at all viewports: +148px at 1920, +96px at 1280, +76px at 1024. Element has `position: fixed` so overflow is cosmetic but contributes to layout instability.
- IMPACT: Background element exceeds viewport bounds; low visual impact due to fixed positioning but may trigger horizontal scrollbar on some browsers.

### terminal-03
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has `overflow: hidden` and clips content at all viewports. scrollHeight=1124 vs clientHeight=676 = 448px clipped at 1920x1080. scrollWidth overflow: +330px at 1920, +255px at 1280, +247px at 1024.
- IMPACT: Panel silently clips approximately 448px of vertical content (chat messages, input area may become unreachable as messages accumulate) and 247-330px horizontally. Users cannot scroll to see clipped content.

### terminal-04
- SEVERITY: P2
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: React warning on Computer Use tab click: "Removing borderBottomColor borderBottom" — shorthand `borderBottom` conflicts with `borderBottomColor` in inline styles at `NexusCodePage.tsx:277:35`.
- IMPACT: Style conflict may cause tab border color to not render correctly during re-render; non-blocking but indicates fragile inline style mixing.

### terminal-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Chat" and "Computer Use" tab selectors are plain `div` elements with `cursor: pointer` but no `role="tab"`, no `tabindex`, no `aria-selected`. Parent container lacks `role="tablist"`.
- IMPACT: Tab interface is invisible to assistive technology; keyboard-only users cannot tab to or activate these controls.

### terminal-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `input[placeholder="Ask the governed agent..."]` has no `id`, no `aria-label`, no associated `<label>`. Relies solely on `placeholder` for identification.
- IMPACT: Screen readers announce input as unlabeled; placeholder text disappears on focus, leaving no persistent label for AT users.

### terminal-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button click produces no visible change, no console output, no network request. Silent no-op in demo mode.
- IMPACT: Button appears clickable but provides zero feedback — user cannot tell if action was attempted or ignored.

### terminal-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button click produces no visible change, no console output, no network request. Silent no-op in demo mode.
- IMPACT: Button appears clickable but provides zero feedback — user cannot tell if action was attempted or ignored.

### terminal-09
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Send" button has `type=null` (no `type` attribute set). Should be `type="button"` to prevent implicit form submission behavior.
- IMPACT: Without explicit `type="button"`, the button defaults to `type="submit"` which could trigger unexpected form submission if wrapped in a form element in the future.

## Summary
- Gate detected: no
- Total interactive elements: 9
- Elements clicked: 8
- P0: 0
- P1: 2
- P2: 7
