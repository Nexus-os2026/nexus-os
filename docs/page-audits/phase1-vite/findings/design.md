# Audit: Design
URL: http://localhost:1420/design
Audited at: 2026-04-09T23:25:00Z
Gate detected: true
Gate type: RequiresLlm

## Console (captured at 1888x951, ALL messages)
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

### 1920x1080 (actual viewport locked at 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW by 151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW by 139px, masked by overflow:hidden]

### 1280x800
- resize_window had no effect — viewport remained locked at 1888x951
- measurements identical to 1920x1080 above

### 1024x768
- resize_window had no effect — viewport remained locked at 1888x951
- measurements identical to 1920x1080 above

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Install Ollama (Free, Local, Private) | button (no type attr) | — | yes |
| 2 | I have an API key (OpenAI, Anthropic, etc.) | a | #/settings | yes |

### Header buttons (outside sidebar, outside main)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 3 | Refresh | button (no type attr) | — | yes |
| 4 | Start Jarvis | button (no type attr) | — | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /design
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /design
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /design
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /design
- Reverted: n/a

### Click 3: "Install Ollama (Free, Local, Private)"
- Pathname before: /design
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /design
- Reverted: n/a

### Click 4: "I have an API key (OpenAI, Anthropic, etc.)"
- Pathname before: /design
- New console: clean
- Network failures: none
- Visible change: hash changed to #/settings, gate screen remained visible, no navigation to /settings
- Pathname after: /design (hash: #/settings)
- Reverted: yes (cleared hash)

### Skipped (destructive)
none

### Total interactive elements found: 4 (2 main + 2 header)
### Elements clicked: 4 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### design-01
- SEVERITY: P1
- DIMENSION: gate
- VIEWPORT: all
- EVIDENCE: `a[href="#/settings"]` inside the RequiresLlm gate links to hash route `#/settings`. Clicking appends `#/settings` to current URL (`/design#/settings`) instead of navigating to `/settings`. The gate screen remains visible — user cannot reach settings.
- IMPACT: Users who click "I have an API key" cannot reach the settings page to configure their provider.

### design-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` has scrollWidth=2039 vs clientWidth=1888 at 1888x951 viewport. Element uses `position:fixed` so it exceeds the viewport width by 151px.
- IMPACT: Background element overflows viewport; hidden by body overflow clipping but may cause horizontal scroll on some browsers/OS combinations.

### design-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` has scrollWidth=1716 vs clientWidth=1577 (overflow by 139px). Masked by `overflow:hidden` on the section.
- IMPACT: Internal content is clipped rather than properly sized; content may be hidden from users.

### design-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Three buttons missing `type` attribute: "Refresh" (header), "Start Jarvis" (header), "Install Ollama (Free, Local, Private)" (main gate). All render as implicit `type="submit"`.
- IMPACT: Buttons without explicit `type="button"` may trigger unintended form submission if wrapped in a form element.

### design-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (header), "Start Jarvis" button (header), and "Install Ollama" button (gate) are all silent no-ops — clicking produces no console output, no network requests, no visible feedback.
- IMPACT: Three of four interactive elements on the page do nothing when clicked, with zero user feedback.

### design-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The gate region in `main` uses a generic `div` with `role="region"` (per accessibility tree) but has no `aria-label` or `aria-labelledby`. The `section.holo-panel` also lacks `role` and `aria-label`. The `<header>` element has no `role="banner"`.
- IMPACT: Screen readers cannot identify or announce the gate section or header landmark meaningfully.

## Summary
- Gate detected: yes
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 1
- P2: 5
