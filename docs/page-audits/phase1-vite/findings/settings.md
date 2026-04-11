# Audit: Settings
URL: http://localhost:1420/settings
Audited at: 2026-04-09T19:47:00Z
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
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1891 clientWidth=1577 [OVERFLOW +314px, hidden by overflow:hidden]
  - `div#claude-static-indicator-container`: scrollWidth=368 clientWidth=357 [OVERFLOW +11px — browser extension]
  - `button#claude-static-chat-button`: scrollWidth=56 clientWidth=32 [OVERFLOW — browser extension]
  - `button#claude-static-close-button`: scrollWidth=50 clientWidth=32 [OVERFLOW — browser extension]

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1346 clientWidth=1248 [OVERFLOW +98px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1133 clientWidth=937 [OVERFLOW +196px, hidden by overflow:hidden]
  - `div#claude-static-indicator-container`: scrollWidth=368 clientWidth=357 [OVERFLOW — browser extension]

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1069 clientWidth=992 [OVERFLOW +77px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=915 clientWidth=681 [OVERFLOW +234px, hidden by overflow:hidden]
  - `div#claude-static-indicator-container`: scrollWidth=368 clientWidth=357 [OVERFLOW — browser extension]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | General | button[type=button] | — | yes |
| 2 | LLM Providers | button[type=button] | — | yes |
| 3 | API Keys | button[type=button] | — | yes |
| 4 | Privacy | button[type=button] | — | yes |
| 5 | Voice | button[type=button] | — | yes |
| 6 | Models | button[type=button] | — | yes |
| 7 | Tools | button[type=button] | — | yes |
| 8 | About | button[type=button] | — | yes |
| 9 | (Dark mode toggle) | input[type=checkbox] | — | yes |
| 10 | (Language) | select | — | yes |
| 11 | (Desktop Notifications toggle) | input[type=checkbox] | — | yes |
| 12 | (UI Sound Design toggle) | input[type=checkbox] | — | yes |
| 13 | (Volume slider) | input[type=range] | — | yes |
| 14 | (Warden Governance toggle) | input[type=checkbox] | — | yes |
| 15 | Save Settings | button[type=button] | — | yes |

Note: Refresh and Start Jarvis buttons are in page header, outside `<main>`.

## Click sequence
### Click 1: "General" (tab button)
- Pathname before: /settings
- New console: clean
- Network failures: none
- Visible change: none (already active tab)
- Pathname after: /settings
- Reverted: n/a

### Click 2: "LLM Providers" (tab button)
- Pathname before: /settings
- New console: clean
- Network failures: none
- Visible change: Tab highlight moves to LLM Providers (green border + text); content switches to Active Provider / Routing Strategy / Provider Status list
- Pathname after: /settings
- Reverted: n/a

### Click 3: "API Keys" (tab button)
- Pathname before: /settings
- New console: clean
- Network failures: none
- Visible change: Content switches to Show Keys / OpenAI / Anthropic key management
- Pathname after: /settings
- Reverted: n/a

### Click 4: Dark Mode toggle (checkbox)
- Pathname before: /settings
- New console: clean
- Network failures: none
- Visible change: Checkbox unchecks; label changes from "Dark" (no other visible theme change observed)
- Pathname after: /settings
- Reverted: n/a (toggled back manually)

### Click 5: Desktop Notifications toggle (checkbox)
- Pathname before: /settings
- New console: clean
- Network failures: none
- Visible change: Toggle activates (checked becomes true)
- Pathname after: /settings
- Reverted: n/a

### Click 6: UI Sound Design toggle (checkbox)
- Pathname before: /settings
- New console: clean
- Network failures: none
- Visible change: Toggle activates; volume range slider remains enabled (was already enabled before toggle)
- Pathname after: /settings
- Reverted: n/a

### Click 7: Warden Governance Review toggle (checkbox)
- Pathname before: /settings
- New console: clean
- Network failures: none
- Visible change: Toggle activates; label text changes from "Off" to "On"
- Pathname after: /settings
- Reverted: n/a

### Click 8: "Save Settings" (button)
- Pathname before: /settings
- New console: clean
- Network failures: none
- Visible change: none — no success message, no toast, no error, completely silent
- Pathname after: /settings
- Reverted: n/a

### Click 9: "Refresh" (header button)
- Pathname before: /settings
- New console: clean
- Network failures: none
- Visible change: none — silent no-op in demo mode
- Pathname after: /settings
- Reverted: n/a

### Click 10: "Start Jarvis" (header button)
- Pathname before: /settings
- New console: clean
- Network failures: none
- Visible change: none — silent no-op in demo mode
- Pathname after: /settings
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 15 (main) + 2 (header)
### Elements clicked: 10

## Accessibility
- Images without alt: 0
- Inputs without label: 4
  - `select[type=select-one].st-select` — Language dropdown; no id, no aria-label, no associated label element
  - `input[type=checkbox]` in `label.st-toggle` — Desktop Notifications; label wraps checkbox but contains no text content
  - `input[type=checkbox]` in `label.st-toggle` — UI Sound Design; label wraps checkbox but contains no text content
  - `input[type=range].st-slider` — Volume slider; no label, no aria-label, no associated label element
- Buttons without accessible name: 0
- Tab navigation buttons lack ARIA tab pattern: 8 buttons with no `role="tab"`, no `aria-selected`, no `aria-controls`; parent container has no `role="tablist"`

## Findings

### settings-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has `overflow: hidden` and clips content at all viewports. At 1920x1080: scrollHeight=1537 vs clientHeight=637 (900px clipped vertically), scrollWidth=1891 vs clientWidth=1577 (314px clipped horizontally). At 1280x800: scrollHeight=795 vs clientHeight=297 (498px clipped). At 1024x768: scrollHeight=438 vs clientHeight=265 (173px clipped). Settings content below the fold is completely invisible.
- IMPACT: Users cannot scroll to see all settings within the panel; the Save Settings button and lower form controls may be clipped from view at smaller viewports.

### settings-02
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 8 tab navigation buttons (`General`, `LLM Providers`, `API Keys`, `Privacy`, `Voice`, `Models`, `Tools`, `About`) have no `role="tab"`, no `aria-selected`, no `aria-controls`. Parent container has no `role="tablist"`. Active state is communicated only via CSS class `.active` (green border/text). Example: `<button class="st-nav-btn cursor-pointer active" type="button">LLM Providers</button>`.
- IMPACT: Screen readers cannot identify the tab pattern, active tab, or tab-panel association. WCAG 4.1.2 violation.

### settings-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 4 form inputs have no programmatic label. (1) `select.st-select` (Language) — no `id`/`for`, no `aria-label`. (2) `input[type=checkbox]` in `.st-toggle` (Desktop Notifications) — wrapping `<label>` contains no text. (3) `input[type=checkbox]` in `.st-toggle` (UI Sound Design) — same issue. (4) `input[type=range].st-slider` (Volume) — no label, no `aria-label`.
- IMPACT: Screen readers announce these controls without any identifying label. WCAG 1.3.1 / 4.1.2 violation.

### settings-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows at all viewports: 1920x1080 (+151px), 1280x800 (+98px), 1024x768 (+77px). This is a decorative background layer.
- IMPACT: Does not cause visible scrollbar (parent clips), but contributes to layout width calculation inconsistency across the app.

### settings-05
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Save Settings" button click produces no visible feedback — no console output, no toast/notification, no button text change, no success/error message. In demo mode the click is a complete silent no-op.
- IMPACT: User has no confirmation that their settings changes were accepted or rejected.

### settings-06
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Refresh" button (header) click produces no console output or visible effect in demo mode. "Start Jarvis" button (header) is also a silent no-op with no console or visible feedback.
- IMPACT: Users clicking these buttons receive no indication of success, failure, or demo-mode limitation.

### settings-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `span.nexus-sidebar-item-text` elements (x3) overflow by 4px each (scrollWidth=157, clientWidth=153). Sidebar nav text is being clipped.
- IMPACT: Some sidebar navigation labels may have truncated text without visible ellipsis or tooltip.

## Summary
- Gate detected: no
- Total interactive elements: 17 (15 in main + 2 in header)
- Elements clicked: 10
- P0: 0
- P1: 1
- P2: 6
