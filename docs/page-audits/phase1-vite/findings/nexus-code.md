# Audit: Nexus Code
URL: http://localhost:1420/nexus-code
Audited at: 2026-04-09T20:05:00Z
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
1. `TypeError: Cannot read properties of undefined (reading 'transformCallback')` at chunk-G7S6KQDI.js:22 via @tauri-apps_api_event.js:38 — triggered from NexusCode.tsx:228 (listen call in useEffect) at NexusCode.tsx:364. Tauri event bridge unavailable in demo mode. (fires twice — React StrictMode double-invoke)

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
- documentElement: scrollWidth=1920 clientWidth=1888 [OVERFLOW +32px — window chrome delta]
- body: scrollWidth=1920 clientWidth=1920 [OK]
- main `main.nexus-shell-content`: scrollWidth=1659 clientWidth=1659 [OK]
- `section.holo-panel.holo-panel-mid`: scrollWidth=1924 clientWidth=1609 [OVERFLOW +315px]
- `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px — fixed-position background]

### 1280x800
- documentElement: scrollWidth=1280 clientWidth=1280 [OK — simulated]
- body: scrollWidth=1280 clientWidth=1280 [OK]
- main `main.nexus-shell-content`: scrollWidth=1019 clientWidth=1019 [OK]
- `section.holo-panel.holo-panel-mid`: scrollWidth=1231 clientWidth=969 [OVERFLOW +262px]

### 1024x768
- documentElement: scrollWidth=1024 clientWidth=1024 [OK — simulated]
- body: scrollWidth=1024 clientWidth=1024 [OK]
- main `main.nexus-shell-content`: scrollWidth=763 clientWidth=763 [OK]
- `section.holo-panel.holo-panel-mid`: scrollWidth=945 clientWidth=713 [OVERFLOW +232px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | (placeholder: "Ask the governed agent...") | input[text] | — | true |
| 2 | Send | button[submit] | — | false (disabled until input has text) |

## Click sequence
### Click 1: "Ask the governed agent..." (input)
- Pathname before: /nexus-code
- New console: clean
- Network failures: none
- Visible change: Input field focused, cursor appears
- Pathname after: /nexus-code
- Reverted: n/a

### Click 2: "Send" (button — after typing "hello world")
- Pathname before: /nexus-code
- New console: clean (no new errors)
- Network failures: none
- Visible change: User message "hello worldhello world" rendered in chat area. Error response displayed: "Failed: Error: desktop runtime unavailable". Send button re-enabled after error.
- Pathname after: /nexus-code
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 2
### Elements clicked: 2

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `input[type=text][placeholder="Ask the governed agent..."]` — has placeholder only, no `<label>`, no `aria-label`, no `aria-labelledby`)
- Buttons without accessible name: 0
- Status indicator "✗" (next to AUDIT 0) has no `aria-label` or `title` — screen readers cannot interpret its meaning

## Findings

### nexus-code-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: `TypeError: Cannot read properties of undefined (reading 'transformCallback')` at chunk-G7S6KQDI.js:22, called from NexusCode.tsx:228 via the `listen()` import from `@tauri-apps/api/event`. The useEffect at line 244-367 calls `listen()` for events `nx:text-delta`, `nx:tool-start`, `nx:turn-end`, `nx:error`, `nx:consent-request` without guarding against missing Tauri runtime. Fires twice (React StrictMode).
- IMPACT: Two uncaught TypeErrors on every page load. Event subscriptions silently fail, meaning even if a Tauri-like bridge were mocked, no streaming data would be received.

### nexus-code-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` has `overflow: hidden`, `height: 694.719px` (fixed), `scrollHeight: 1278px`, `scrollWidth: 1924px` at 1920 viewport. At 1280: scrollWidth=1231 vs clientWidth=969 (+262px). At 1024: scrollWidth=945 vs clientWidth=713 (+232px). Content is clipped both horizontally (315px at 1920) and vertically (585px at 1920).
- IMPACT: The holo-panel clips ~585px of vertical content and 315px of horizontal content. Any chat messages, provider picker, or consent modals that render below the visible fold are invisible to the user with no scroll mechanism.

### nexus-code-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `input[type=text][placeholder="Ask the governed agent..."]` has no `<label>` element, no `aria-label`, and no `aria-labelledby`. Relies solely on `placeholder` for identification. Source: NexusCode.tsx:687-699.
- IMPACT: Screen readers announce the input as an unnamed text field. Placeholder text disappears on focus, leaving no persistent label.

### nexus-code-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The "✗" span adjacent to "AUDIT 0" in the governance stats bar has no `aria-label`, `title`, or `role` attribute. It is a bare `<span>` with a Unicode cross mark character.
- IMPACT: Screen readers will either skip it or announce "times" with no context — the operational meaning (audit chain status: disconnected/failed) is lost for assistive technology users.

### nexus-code-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Clicking Send renders "Failed: Error: desktop runtime unavailable" in the chat area. The error string is a raw internal exception message exposed directly to the user.
- IMPACT: In demo mode, the error message is technically accurate but not user-friendly. The same raw error pattern would surface for any Tauri command failure in production if not wrapped.

### nexus-code-06
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: The inline `<style>` tag containing `@keyframes blink` is injected inside the `main` content element (NexusCode.tsx:819-823). It renders as a child of the component's root `<div>`.
- IMPACT: Injecting `<style>` inside `<main>` is non-standard. While browsers tolerate it, it pollutes the content DOM and could interfere with CSS scoping or SSR. Should be in `<head>` or use CSS-in-JS.

### nexus-code-07
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` (position: fixed) has scrollWidth=2039 vs clientWidth=1888 at 1920 viewport (+151px overflow). This is a decorative background layer.
- IMPACT: While the fixed-position background overflow is hidden by `overflow: hidden` on ancestors, it contributes unnecessary layout width that could cause horizontal scrollbars if any ancestor changes overflow behavior.

## Summary
- Gate detected: no
- Total interactive elements: 2
- Elements clicked: 2
- P0: 0
- P1: 2
- P2: 5
