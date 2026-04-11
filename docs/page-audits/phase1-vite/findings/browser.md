# Audit: Browser
URL: http://localhost:1420/browser
Audited at: 2026-04-09T21:44:00Z
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
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js:21550:24

### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2035 clientWidth=1888 [OVERFLOW +147px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=2108 clientWidth=1577 [OVERFLOW +531px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1344 clientWidth=1248 [OVERFLOW +96px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1042 clientWidth=937 [OVERFLOW +105px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1067 clientWidth=992 [OVERFLOW +75px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=875 clientWidth=681 [OVERFLOW +194px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Research | button | — | yes |
| 2 | Build | button | — | yes |
| 3 | Learn | button | — | yes |
| 4 | ⏲ (title: "History (Ctrl+H)") | button | — | yes |
| 5 | ⚔ (title: "Governance (Ctrl+G)") | button | — | yes |
| 6 | ◁ (title: "Back") | button | — | no (disabled) |
| 7 | ▷ (title: "Forward") | button | — | no (disabled) |
| 8 | ↻ (title: "Refresh (Ctrl+R)") | button | — | yes |
| 9 | (placeholder: "Enter URL... (Ctrl+L to focus)") | input[text] | — | yes |
| 10 | → (title: "Navigate") | button | — | yes |

## Click sequence

### Click 1: "Research"
- Pathname before: /browser
- New console: clean
- Network failures: none
- Visible change: "Research" tab gains `active` class; content unchanged (Playwright setup message persists)
- Pathname after: /browser
- Reverted: n/a

### Click 2: "Build"
- Pathname before: /browser
- New console: clean
- Network failures: none
- Visible change: "Build" tab gains `active` class; "Research" loses it; content unchanged
- Pathname after: /browser
- Reverted: n/a

### Click 3: "Learn"
- Pathname before: /browser
- New console: clean
- Network failures: none
- Visible change: "Learn" tab gains `active` class; content unchanged
- Pathname after: /browser
- Reverted: n/a

### Click 4: "⏲ History"
- Pathname before: /browser
- New console: clean
- Network failures: none
- Visible change: `div.browser-history-dropdown` appears with "Browsing History" header, "No pages visited yet" message, and "x" close button
- Pathname after: /browser
- Reverted: n/a

### Click 5: "⚔ Governance"
- Pathname before: /browser
- New console: clean
- Network failures: none
- Visible change: `div.governance-sidebar` appears with stats (0 Domains Blocked, 0 PII Redactions, 0 Fuel Consumed, 0 Audit Events) and "Desktop runtime unavailable" message
- Pathname after: /browser
- Reverted: n/a

### Click 6: "↻ Refresh"
- Pathname before: /browser
- New console: clean
- Network failures: none
- Visible change: none observed (no page loaded to refresh)
- Pathname after: /browser
- Reverted: n/a

### Click 7: "→ Navigate"
- Pathname before: /browser
- New console: clean
- Network failures: none
- Visible change: none observed (URL input empty, nothing to navigate to)
- Pathname after: /browser
- Reverted: n/a

### Click 8: URL input (focus/click)
- Pathname before: /browser
- New console: clean
- Network failures: none
- Visible change: input receives focus
- Pathname after: /browser
- Reverted: n/a

### Skipped (disabled)
- "◁ Back" — reason: disabled
- "▷ Forward" — reason: disabled

### Skipped (destructive)
none

### Total interactive elements found: 10
### Elements clicked: 8 (2 disabled, skipped)

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `input[type=text].browser-url-input` — has placeholder only, no id, no aria-label, no associated label element)
- Buttons without accessible name: 0 (all buttons have text content or title attribute)

## Findings

### browser-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` has `overflow: hidden` and scrollWidth exceeds clientWidth at all viewports: 2108 vs 1577 (+531px) at 1920, 1042 vs 937 (+105px) at 1280, 875 vs 681 (+194px) at 1024. scrollHeight=1268 vs clientHeight=637 (+631px vertical). Content is silently clipped.
- IMPACT: Browser page content inside holo-panel is clipped on both axes; users cannot scroll to see off-screen elements.

### browser-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` scrollWidth exceeds clientWidth at all viewports: 2035 vs 1888 (+147px) at 1920, 1344 vs 1248 (+96px) at 1280, 1067 vs 992 (+75px) at 1024.
- IMPACT: Decorative background element overflows viewport; may cause layout shift if overflow containment changes.

### browser-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `input[type=text].browser-url-input` has no `id`, no `aria-label`, no `aria-labelledby`, no associated `<label>` element. Only has `placeholder="Enter URL... (Ctrl+L to focus)"` which is not a sufficient accessible name per WCAG 2.1 SC 1.3.1.
- IMPACT: Screen readers cannot programmatically determine the purpose of the URL input field.

### browser-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `h1` element ("Agent Browser") is located outside `<main>`, inside `div.flex.flex-wrap` in the shell header area. `<main>` has no heading.
- IMPACT: Landmark navigation gives `<main>` no heading; screen reader users cannot identify the primary content region by heading.

### browser-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Mode tab buttons (Research, Build, Learn) lack ARIA tab pattern: no `role="tab"`, no `aria-selected`, no `aria-controls`, parent has no `role="tablist"`. Active state is communicated only via CSS class `active`.
- IMPACT: Screen readers cannot convey tab selection state; keyboard users have no programmatic way to know which mode is active.

### browser-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: History close button (`button.browser-history-close`, text "x") has no `aria-label`, no `title`, and no `type` attribute. The single-character "x" text content is a poor accessible name.
- IMPACT: Screen readers announce "x button" with no context; the button's purpose (close history panel) is not programmatically conveyed.

### browser-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: 4 buttons page-wide missing `type` attribute: "Refresh" (shell header), "Start Jarvis" (shell header), "x" (history close, in main), "Open chat" and "Dismiss" (Claude extension). Without `type="button"`, these default to `type="submit"` inside forms.
- IMPACT: If any of these buttons end up inside a `<form>` ancestor, they will trigger form submission instead of their intended action.

## Summary
- Gate detected: no
- Total interactive elements: 10
- Elements clicked: 8
- P0: 0
- P1: 0
- P2: 7
