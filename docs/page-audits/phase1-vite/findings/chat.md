# Audit: Chat
URL: http://localhost:1420/chat
Audited at: 2026-04-10T01:42:00Z
Gate detected: true
Gate type: RequiresLlm

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

> **Note:** Viewport locked at 1888x951 for all resize attempts. resize_window API reports success but `window.innerWidth`/`window.innerHeight` remain unchanged. Measurements below are all at the actual 1888x951 viewport.

### 1920x1080 (actual 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content.chat-active`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1750 clientWidth=1609 [OVERFLOW +141px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px, text truncation in sidebar]

### 1280x800 (actual 1888x951 — resize failed)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content.chat-active`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as above (viewport unchanged)

### 1024x768 (actual 1888x951 — resize failed)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content.chat-active`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as above (viewport unchanged)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Install Ollama (Free, Local, Private) | button | — | yes |
| 2 | I have an API key (OpenAI, Anthropic, etc.) | a (link) | #/settings | yes |

## Click sequence
### Click 1: "Install Ollama (Free, Local, Private)"
- Pathname before: /chat
- New console: clean
- Network failures: none
- Visible change: none — silent no-op in demo mode
- Pathname after: /chat
- Reverted: n/a

### Click 2: "I have an API key (OpenAI, Anthropic, etc.)"
- Pathname before: /chat
- New console: clean
- Network failures: none
- Visible change: none — link href="#/settings" did not navigate; hash remained empty, page stayed on gate screen
- Pathname after: /chat
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 2
### Elements clicked: 2 (of 2)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### chat-01
- SEVERITY: P1
- DIMENSION: gate
- VIEWPORT: all
- EVIDENCE: The "I have an API key" link has `href="#/settings"`. The app uses browser-history routing (path-based), not hash routing. Clicking the link produces no navigation — pathname stays `/chat`, hash stays empty, page remains on the gate screen. The user has no way to reach the Settings page from the gate CTA.
- IMPACT: Gate screen's primary escape route is broken; user cannot follow the intended flow to configure an API key and unlock Chat.

### chat-02
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: The "Install Ollama" `<button>` has no `type` attribute (`type: null`). Without an explicit `type`, it defaults to `type="submit"`, which may cause unintended form submission if the button is ever placed inside a `<form>`.
- IMPACT: Semantic mismatch; button defaults to submit instead of the intended button type.

### chat-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1888 (locked)
- EVIDENCE: `div.living-background` has scrollWidth=2039 vs clientWidth=1888, a 151px horizontal overflow. This is a decorative/background element that bleeds beyond the viewport edge.
- IMPACT: May cause a horizontal scrollbar on some browsers/OSes or interfere with layout calculations.

### chat-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1888 (locked)
- EVIDENCE: `section.holo-panel.holo-panel-mid` (the gate card container) has scrollWidth=1750 vs clientWidth=1609, a 141px horizontal overflow.
- IMPACT: Gate card's inner content overflows its container, may cause horizontal scrollbar or content clipping.

### chat-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The page has no `<h1>` element. The only heading is `<h2>Chat needs an AI engine</h2>` inside the gate card. Heading hierarchy skips from none to H2.
- IMPACT: Screen readers and SEO crawlers expect exactly one H1 per page; skipped heading levels violate WCAG 1.3.1.

### chat-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The gate card is wrapped in `<section class="holo-panel holo-panel-mid nexus-page-panel">` with no `role` attribute and no `aria-label` or `aria-labelledby`.
- IMPACT: The section landmark is anonymous to assistive technology; users cannot identify or navigate to it by name.

### chat-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Install Ollama" button click produces no visible feedback, no console output, and no error message in demo mode. The button appears clickable but is a complete no-op.
- IMPACT: User receives no indication that the action failed or is unavailable in demo mode; violates the principle of least surprise.

## Summary
- Gate detected: yes
- Total interactive elements: 2
- Elements clicked: 2
- P0: 0
- P1: 1
- P2: 6
