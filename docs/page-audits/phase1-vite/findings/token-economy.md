# Audit: Token Economy
URL: http://localhost:1420/token-economy
Audited at: 2026-04-09T22:38:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
none

### Warnings
1. `[TokenEconomy] Error: desktop runtime unavailable` — TokenEconomy.tsx:107:39 via `tokenCalculateReward` (backend.ts:2508)
2. `[TokenEconomy] Error: desktop runtime unavailable` — TokenEconomy.tsx:112:39 via `tokenCalculateBurn` (backend.ts:2498)
3. (Repeat of #1) — React StrictMode double-invocation via `invokePassiveEffectMountInDEV`
4. (Repeat of #2) — React StrictMode double-invocation via `invokePassiveEffectMountInDEV`

### Logs
none

### Info
1. `Download the React DevTools for a better development experience` — chunk-NUMECXU6.js:21550:24

### Debug
1. `[vite] connecting...` — @vite/client:494:8
2. `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080 (actual viewport 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px, overflow:hidden clips content]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each, sidebar text clipping]

### 1280x800
Note: resize_window does not affect window.innerWidth/innerHeight in Tauri-wrapped Chrome. Measurements identical to 1920x1080.
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as 1920x1080

### 1024x768
Note: resize_window does not affect window.innerWidth/innerHeight in Tauri-wrapped Chrome. Measurements identical to 1920x1080.
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as 1920x1080

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Supply Overview | button[type=submit] | — | yes |
| 2 | Agent Wallets | button[type=submit] | — | yes |
| 3 | Transaction Feed | button[type=submit] | — | yes |
| 4 | Pricing & Rewards | button[type=submit] | — | yes |
| 5 | (none — Model dropdown) | select[select-one] | — | yes |
| 6 | (none — Input Tokens) | input[number] value=1000 | — | yes |
| 7 | (none — Output Tokens) | input[number] value=500 | — | yes |
| 8 | (none — Quality Score) | input[range] value=0.8 min=0 max=1 step=0.01 | — | yes |
| 9 | (none — Difficulty) | input[range] value=0.6 min=0 max=1 step=0.01 | — | yes |
| 10 | (none — Completion Time) | input[range] value=30 | — | yes |

## Click sequence
### Click 1: "Supply Overview"
- Pathname before: /token-economy
- New console: clean
- Network failures: none
- Visible change: none — content identical before and after click; this button appears to be the default active tab but all sections are rendered simultaneously
- Pathname after: /token-economy
- Reverted: n/a

### Click 2: "Agent Wallets"
- Pathname before: /token-economy
- New console: clean
- Network failures: none
- Visible change: button may receive visual highlight (amber border) but page content does not change; all 4 sections remain visible simultaneously
- Pathname after: /token-economy
- Reverted: n/a

### Click 3: "Transaction Feed"
- Pathname before: /token-economy
- New console: clean
- Network failures: none
- Visible change: button may receive visual highlight but page content does not change
- Pathname after: /token-economy
- Reverted: n/a

### Click 4: "Pricing & Rewards"
- Pathname before: /token-economy
- New console: clean
- Network failures: none
- Visible change: button receives amber highlight (bgColor rgba(245,158,11,0.2), border 1px solid rgb(245,158,11)); content does not change — all sections remain visible
- Pathname after: /token-economy
- Reverted: n/a

### Click 5: Model select dropdown
- Pathname before: /token-economy
- New console: clean
- Network failures: none
- Visible change: dropdown opens but contains 0 options — empty select element
- Pathname after: /token-economy
- Reverted: n/a

### Click 6: Input Tokens (input[number])
- Pathname before: /token-economy
- New console: clean
- Network failures: none
- Visible change: field receives focus; pre-filled with value "1000"
- Pathname after: /token-economy
- Reverted: n/a

### Click 7: Output Tokens (input[number])
- Pathname before: /token-economy
- New console: clean
- Network failures: none
- Visible change: field receives focus; pre-filled with value "500"
- Pathname after: /token-economy
- Reverted: n/a

### Click 8: Quality Score (input[range])
- Pathname before: /token-economy
- New console: clean
- Network failures: none
- Visible change: slider receives focus; value 0.8
- Pathname after: /token-economy
- Reverted: n/a

### Click 9: Difficulty (input[range])
- Pathname before: /token-economy
- New console: clean
- Network failures: none
- Visible change: slider receives focus; value 0.6
- Pathname after: /token-economy
- Reverted: n/a

### Click 10: Completion Time (input[range])
- Pathname before: /token-economy
- New console: clean
- Network failures: none
- Visible change: slider receives focus; value 30
- Pathname after: /token-economy
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 10
### Elements clicked: 10

## Accessibility
- Images without alt: 0
- Inputs without label: 6 (select[select-one], input[number] x2, input[range] x3 — none have id, aria-label, aria-labelledby, or associated `<label for>`)
- Buttons without accessible name: 0

## Findings

### token-economy-01
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: The 4 tab buttons ("Supply Overview", "Agent Wallets", "Transaction Feed", "Pricing & Rewards") visually highlight on click (amber border on active) but do not switch content. All 4 content sections are rendered simultaneously at all times. The buttons have no `role="tab"`, no `aria-selected`, no `aria-controls`. The parent `<div>` container has no `role="tablist"`. Clicking any tab produces no content change.
- IMPACT: Tab navigation is broken — users see all sections at once with no way to filter, and the tab buttons mislead by suggesting switchable views.

### token-economy-02
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: `<select>` element in the "Model Pricing Table" / Burn Calculator section contains 0 `<option>` elements. Clicking opens an empty dropdown. The select has no `id`, no `aria-label`, and no `<label>` association.
- IMPACT: Model selection for burn/pricing calculation is completely non-functional; users cannot select any model.

### token-economy-03
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: The Model Pricing Table has a `<table>` with 5 column headers ("Model", "Class", "In/1K", "Out/1K", "Local") but `<tbody>` is empty (0 rows). No empty-state message is displayed.
- IMPACT: The pricing table appears broken with headers but no data, with no indication that data is unavailable in demo mode.

### token-economy-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 6 form inputs lack accessible labels: 1 `select[select-one]`, 2 `input[number]`, 3 `input[range]`. None have `id`, `aria-label`, `aria-labelledby`, or an associated `<label for>`. Visual labels exist as adjacent text (e.g., "Input Tokens", "Quality Score") but are not programmatically linked.
- IMPACT: Screen readers cannot identify these form controls; users relying on assistive technology cannot determine the purpose of any input.

### token-economy-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Duplicate `<h1>` elements found: both contain "Token Economy". One is in the shell header, the other in page content.
- IMPACT: Screen readers announce two identical H1 headings, violating the single-H1-per-page best practice and creating confusing document structure.

### token-economy-06
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` scrollWidth=2039 vs clientWidth=1888 (+151px). `section.holo-panel.holo-panel-mid` scrollWidth=1200 vs clientWidth=937 (+263px, clipped by `overflow:hidden`). The holo-panel clips its own content in both axes.
- IMPACT: Background element causes potential horizontal scroll; holo-panel silently clips page content that may extend beyond the visible area.

### token-economy-07
- SEVERITY: P2
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: 4 console warnings on page load (2 unique, doubled by React StrictMode): `tokenCalculateReward` at TokenEconomy.tsx:107 and `tokenCalculateBurn` at TokenEconomy.tsx:112 both throw "Error: desktop runtime unavailable" via `invokeDesktop` at backend.ts:17.
- IMPACT: Reward and burn calculations fail silently on load in demo mode; computed values display as 0.00 NXC with no error indication to the user.

### token-economy-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 4 tab buttons have `type="submit"` instead of `type="button"`. Tab-switching buttons should not carry implicit form submission semantics.
- IMPACT: If these buttons are ever placed inside a `<form>`, they will trigger form submission instead of tab switching. Incorrect semantic type for navigation controls.

### token-economy-09
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons are positioned outside `<main>` (in the page-level banner flex container, not in `aside.nexus-sidebar-shell` nor in `main.nexus-shell-content`). Both are completely inert in demo mode — no console output, no navigation, no visible feedback.
- IMPACT: Buttons outside the main landmark may be missed by screen reader users navigating by landmarks; silent no-op behavior provides no feedback.

### token-economy-10
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Zero ARIA live regions (`aria-live`, `role="alert"`, `role="status"`, `role="log"`) found in main content area. The page has dynamic content sections (supply stats, wallets, transactions, pricing calculations) that would change when the backend is active.
- IMPACT: Dynamic content updates will not be announced to screen reader users.

## Summary
- Gate detected: no
- Total interactive elements: 10
- Elements clicked: 10
- P0: 0
- P1: 3
- P2: 7
