# Audit: Ai Chat
URL: http://localhost:1420/ai-chat
Audited at: 2026-04-09T18:48:00Z
Gate detected: true
Gate type: RequiresLlm

## Console (captured at 1920x1080, ALL messages)

Note: actual viewport locked at 1248x671 (MCP resize ineffective; see dashboard audit obs 107).

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

Note: viewport locked at 1248x671. MCP resize and JS `window.resizeTo()` are both ineffective in this environment (confirmed in dashboard audit). Measurements below are at the actual viewport. The three target sizes are recorded with extrapolation notes.

### 1248x671 (actual viewport)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- `section.holo-panel`: scrollWidth=1019 clientWidth=937 — internal overflow but `overflow:hidden` clips it [OK, not user-visible]
- `div.living-background`: scrollWidth=1348 clientWidth=1248 — `position:fixed; overflow:hidden`, decorative background [OK, not user-visible]
- `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 — sidebar text truncation, outside main content [EXCLUDED]

### 1920x1080
Not measurable — viewport locked. No horizontal scrollbar observed at 1248; wider viewport would have more room, so no overflow expected.

### 1280x800
Not measurable — viewport locked. At 1248 (close to 1280) no overflow is present.

### 1024x768
Not measurable — viewport locked. The `holo-panel` internal overflow (1019 vs 937) is clipped by `overflow:hidden`. At 1024 the main area would be narrower (~763px), so internal overflow would increase but remains clipped.

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Install Ollama (Free, Local, Private) | button | — | true |
| 2 | I have an API key (OpenAI, Anthropic, etc.) | a | #/settings | true |

## Click sequence

### Click 1: "Install Ollama (Free, Local, Private)"
- Pathname before: /ai-chat
- New console: clean
- Network failures: none
- Visible change: none — button is a silent no-op
- Pathname after: /ai-chat
- Reverted: n/a

### Click 2: "I have an API key (OpenAI, Anthropic, etc.)"
- Pathname before: /ai-chat
- New console: clean
- Network failures: none
- Visible change: none — URL changed to /ai-chat#/settings but page content unchanged; no navigation to /settings occurred
- Pathname after: /ai-chat (hash changed to #/settings)
- Reverted: n/a (cleared hash manually)

### Skipped (destructive)
none

### Total interactive elements found: 2
### Elements clicked: 2

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### ai-chat-01
- SEVERITY: P1
- DIMENSION: gate
- VIEWPORT: all
- EVIDENCE: The "Install Ollama (Free, Local, Private)" `<button>` in the RequiresLlm gate card has no click handler and produces zero side-effects (no console output, no navigation, no network request, no visible change). `button.className = "cursor-pointer"`, `onclick = null`, `disabled = false`.
- IMPACT: Users clicking the primary CTA to install Ollama get no feedback — the button appears broken. Expected behavior: open Ollama download page or show installation instructions.

### ai-chat-02
- SEVERITY: P1
- DIMENSION: gate
- VIEWPORT: all
- EVIDENCE: The "I have an API key" link uses `href="#/settings"` which resolves to `http://localhost:1420/ai-chat#/settings`. The app uses path-based routing (e.g., `/dashboard`, `/agents`, `/settings`). Clicking appends a hash fragment but does NOT navigate to the Settings page. Page content is unchanged after click.
- IMPACT: Users who have an API key cannot reach the Settings page from this gate. The link should use the app router to navigate to `/settings` (path-based, not hash-based).

### ai-chat-03
- SEVERITY: P2
- DIMENSION: gate
- VIEWPORT: all
- EVIDENCE: `section.holo-panel` wrapping the gate card has `scrollWidth=1019` vs `clientWidth=937` (82px internal overflow). The element has `overflow:hidden` so content is clipped rather than scrollable. The panel's `width` is set to `938.812px` but its intrinsic content width is 1019px.
- IMPACT: Currently masked by `overflow:hidden`. If overflow property changes or if content grows, clipped content could become visible or cause layout issues. Low severity since not user-visible today.

## Summary
- Gate detected: yes
- Total interactive elements: 2
- Elements clicked: 2
- P0: 0
- P1: 2
- P2: 1
