# Audit: Messaging
URL: http://localhost:1420/messaging
Audited at: 2026-04-09T20:59:00Z
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
- `Messaging event listener failed: TypeError: Cannot read properties of undefined (reading 'transformCallback')` at Messaging.tsx:115:16 (via @tauri-apps_api_event.js:38:14) — **repeated 2x**
### Warnings
none
### Logs
none
### Info
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` at chunk-NUMECXU6.js:21550:24
### Debug
- `[vite] connecting...` at @vite/client:494:8
- `[vite] connected.` at @vite/client:617:14

## Overflow

### 1920x1080
(actual viewport 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `div#claude-static-indicator-container`: scrollWidth=350 clientWidth=338 [OVERFLOW +12px] (injected by extension)

### 1280x800
(actual viewport 1248x615)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]
  - `div#claude-static-indicator-container`: scrollWidth=368 clientWidth=357 [OVERFLOW +11px] (injected by extension)

### 1024x768
(actual viewport 992x583)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1071 clientWidth=992 [OVERFLOW +79px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=866 clientWidth=681 [OVERFLOW +185px]
  - `div#claude-static-indicator-container`: scrollWidth=368 clientWidth=357 [OVERFLOW +11px] (injected by extension)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | yes |
| 2 | Start Jarvis | button | — | yes |
| 3 | Select agent | select | — | yes |
| 4 | Telegram token | input (text) | — | yes |
| 5 | Connect | button | — | yes |
| 6 | Test | button | — | yes |
| 7 | Test & Connect | button | — | yes |
| 8 | Discord token | input (text) | — | yes |
| 9 | Connect | button | — | yes |
| 10 | Test | button | — | yes |
| 11 | Test & Connect | button | — | yes |
| 12 | Slack token | input (text) | — | yes |
| 13 | Connect | button | — | yes |
| 14 | Test | button | — | yes |
| 15 | Test & Connect | button | — | yes |
| 16 | WhatsApp token | input (text) | — | yes |
| 17 | Connect | button | — | yes |
| 18 | Test | button | — | yes |
| 19 | Test & Connect | button | — | yes |
| 20 | Matrix token | input (text) | — | yes |
| 21 | Connect | button | — | yes |
| 22 | Test | button | — | yes |
| 23 | Test & Connect | button | — | yes |
| 24 | Webhook token | input (text) | — | yes |
| 25 | Connect | button | — | yes |
| 26 | Test | button | — | yes |
| 27 | Test & Connect | button | — | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /messaging
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /messaging
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /messaging
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /messaging
- Reverted: n/a

### Click 3: "Select agent"
- Pathname before: /messaging
- New console: clean
- Network failures: none
- Visible change: dropdown opened briefly; focus not received (document.activeElement !== select)
- Pathname after: /messaging
- Reverted: n/a

### Click 4: "Telegram token" (input)
- Pathname before: /messaging
- New console: clean
- Network failures: none
- Visible change: input did not receive focus (document.activeElement !== input)
- Pathname after: /messaging
- Reverted: n/a

### Click 5: "Connect" (Telegram)
- Pathname before: /messaging
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /messaging
- Reverted: n/a

### Click 6: "Test" (Telegram)
- Pathname before: /messaging
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /messaging
- Reverted: n/a

### Click 7: "Test & Connect" (Telegram)
- Pathname before: /messaging
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /messaging
- Reverted: n/a

### Click 8: "Discord token" (input)
- Pathname before: /messaging
- New console: clean
- Network failures: none
- Visible change: input did not receive focus (document.activeElement !== input)
- Pathname after: /messaging
- Reverted: n/a

### Click 9: "Connect" (Discord)
- Pathname before: /messaging
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /messaging
- Reverted: n/a

### Click 10: "Test" (Discord)
- Pathname before: /messaging
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /messaging
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 27
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 6 (selectors: `input[placeholder="Telegram token"]`, `input[placeholder="Discord token"]`, `input[placeholder="Slack token"]`, `input[placeholder="WhatsApp token"]`, `input[placeholder="Matrix token"]`)
  - 6th: `input[placeholder="Webhook token"]`
- Buttons without accessible name: 0

## Findings

### messaging-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: `Messaging event listener failed: TypeError: Cannot read properties of undefined (reading 'transformCallback')` at Messaging.tsx:115:16, repeated 2x on page load. Tauri event API `listen()` call at line 111 fails because `window.__TAURI_INTERNALS__` is undefined in demo mode.
- IMPACT: Tauri event listeners for real-time messaging updates silently fail; no fallback or graceful degradation visible to user.

### messaging-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows at all viewports: +151px at 1920, +100px at 1280, +79px at 1024. Root cause: `.holo-panel__refraction` is absolutely positioned with `left: -315px` and `width: 2208px`, exceeding parent bounds.
- IMPACT: Cosmetic — parent `.holo-panel` has `overflow: hidden` so no visible scrollbar, but the element is technically oversized in the DOM.

### messaging-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920, 1024
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` overflows: +139px at 1920x1080, +185px at 1024x768. Not detected at 1280x800.
- IMPACT: Potential horizontal scroll if parent overflow containment is removed; currently hidden.

### messaging-04
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 6 token `<input>` elements lack programmatic labels. They have `placeholder` text ("Telegram token", "Discord token", etc.) but no `<label>`, `aria-label`, or `aria-labelledby`. Selectors: `input[placeholder="Telegram token"]`, `input[placeholder="Discord token"]`, `input[placeholder="Slack token"]`, `input[placeholder="WhatsApp token"]`, `input[placeholder="Matrix token"]`, `input[placeholder="Webhook token"]`.
- IMPACT: Screen readers cannot identify input purpose; placeholder disappears on input, leaving no visible label. WCAG 1.3.1 / 4.1.2 violation.

### messaging-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Header buttons "Refresh" and "Start Jarvis" lack explicit `type` attribute. Browser defaults to `type="submit"`. All 6 token `<input>` elements also lack a `type` attribute (defaults to `text`, which is correct but implicit).
- IMPACT: Buttons without `type="button"` may trigger form submission if placed inside a `<form>` in the future; best practice violation.

### messaging-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 18 channel action buttons (Connect, Test, Test & Connect x6) produce no console output, no network requests, and no visible state change when clicked in demo mode. No feedback to indicate the action was attempted or that the backend is unavailable.
- IMPACT: User gets zero feedback on button clicks; unclear whether buttons are wired up or broken.

### messaging-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Header buttons "Refresh" and "Start Jarvis" produce no console output, no network requests, and no visible state change when clicked.
- IMPACT: Same as messaging-06 — no user feedback in demo mode.

### messaging-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 6 token `<input>` elements lack `type` attribute entirely. While browser defaults to `type="text"` which is correct here, explicit typing is a best practice for form controls handling sensitive data (tokens/credentials).
- IMPACT: Minor — inputs should ideally be `type="password"` since they accept API tokens/secrets, which would also prevent shoulder-surfing.

## Summary
- Gate detected: no
- Total interactive elements: 27
- Elements clicked: 10
- P0: 0
- P1: 2
- P2: 6
