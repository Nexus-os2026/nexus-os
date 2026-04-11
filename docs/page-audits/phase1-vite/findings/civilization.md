# Audit: Civilization
URL: http://localhost:1420/civilization
Audited at: 2026-04-10T00:08:00Z
Gate detected: true
Gate type: RequiresLlm

## Console (captured at 1248x671, ALL messages)
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

> **Note:** `resize_window` tool did not change the JavaScript-reported viewport (remained at 1248x671). Measurements below are all at the actual viewport of 1248x671.

### 1248x671 (actual viewport — resize to 1920x1080 did not take effect)
- documentElement: scrollWidth=1248 clientWidth=1248 **OK**
- body: scrollWidth=1248 clientWidth=1248 **OK**
- main `main.nexus-shell-content.px-4.py-4.sm:px-6.sm:py-6`: scrollWidth=987 clientWidth=987 **OK**
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 **OVERFLOW (+100px)**
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1019 clientWidth=937 **OVERFLOW (+82px)**
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 (sidebar text truncation, cosmetic)

### 1280x800
Not measured — resize_window did not change viewport dimensions.

### 1024x768
Not measured — resize_window did not change viewport dimensions.

## Interactive elements (main content only)

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (header) | — | true |
| 2 | Start Jarvis | button (header) | — | true |
| 3 | Install Ollama (Free, Local, Private) | button (gate) | — | true |
| 4 | I have an API key (OpenAI, Anthropic, etc.) | a (gate) | #/settings | true |

## Click sequence

### Click 1: "Refresh"
- Pathname before: /civilization
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /civilization
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /civilization
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /civilization
- Reverted: n/a

### Click 3: "Install Ollama (Free, Local, Private)"
- Pathname before: /civilization
- New console: clean
- Network failures: none
- Visible change: none — silent no-op (expected: should invoke Tauri install command)
- Pathname after: /civilization
- Reverted: n/a

### Click 4: "I have an API key (OpenAI, Anthropic, etc.)"
- Pathname before: /civilization
- New console: clean
- Network failures: none
- Visible change: none — URL changed to /civilization#/settings but page content did not change; gate card still visible
- Pathname after: /civilization#/settings
- Reverted: n/a (hash change only, no real navigation)

### Skipped (destructive)
none

### Total interactive elements found: 4
### Elements clicked: 4

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### civilization-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 1248 (all likely)
- EVIDENCE: `div.living-background` scrollWidth=1348 > clientWidth=1248 (+100px). `section.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=1019 > clientWidth=937 (+82px).
- IMPACT: Horizontal overflow causes hidden content or unintended scrollbar on the page background and main panel.

### civilization-02
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "I have an API key" link uses `href="#/settings"` which navigates to `/civilization#/settings` — a hash fragment on the current page. Page content does not change; the gate card remains visible. Should navigate to `/settings` (the app's Settings page).
- IMPACT: Users clicking the API key setup link are stuck on the gate screen with no way to configure their provider from this page.

### civilization-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button in header bar clicked — no console output, no network request, no visible change. Silent no-op in demo mode.
- IMPACT: Button appears functional but does nothing; no loading indicator or feedback.

### civilization-04
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button in header bar clicked — no console output, no network request, no visible change. Silent no-op in demo mode.
- IMPACT: Button appears functional but does nothing; no loading indicator or feedback.

### civilization-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Install Ollama (Free, Local, Private)" button in gate card clicked — no console output, no visible change. Silent no-op (requires Tauri backend).
- IMPACT: Primary CTA on gate screen does nothing in demo mode; no feedback to user.

### civilization-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Header buttons "Refresh", "Start Jarvis", and gate button "Install Ollama" all lack explicit `type` attribute. `button.type` defaults to `"submit"` per HTML spec. If these buttons are ever placed inside a `<form>`, they will trigger form submission.
- IMPACT: Incorrect semantic type; potential unintended form submission if layout changes.

### civilization-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Sidebar `<nav>` element has no `aria-label` attribute. Page uses `role="banner"` on header but nav is unlabeled.
- IMPACT: Screen reader users cannot distinguish between navigation regions if multiple navs are present.

## Summary
- Gate detected: yes
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 2
- P2: 5
