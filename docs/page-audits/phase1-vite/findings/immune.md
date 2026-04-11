# Audit: Immune
URL: http://localhost:1420/immune
Audited at: 2026-04-10T02:05:00Z
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
- `Download the React DevTools for a better development experience: https://reactjs.org/link/react-devtools` — chunk-NUMECXU6.js:21550:24

### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080 (actual viewport 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content.px-4.py-4.sm:px-6.sm:py-6`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

### 1280x800 (actual viewport 1888x951 — window resize did not affect internal viewport)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as 1920x1080 (viewport locked)

### 1024x768 (actual viewport 1888x951 — window resize did not affect internal viewport)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as 1920x1080 (viewport locked)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Select attacker | select | — | yes |
| 2 | Select defender | select | — | yes |
| 3 | [no label] (number input, value=10) | number | — | yes |
| 4 | Start New Session | button | — | no (disabled) |
| 5 | Run Full Scan | button | — | yes |
| 6 | Enabled (PII Redaction toggle) | button | — | yes |
| 7 | Enabled (API Key Leak Prevention toggle) | button | — | yes |
| 8 | Disabled (IP Address Masking toggle) | button | — | yes |
| 9 | Enabled (Exfiltration Detection toggle) | button | — | yes |
| 10 | Save Privacy Rules | button | — | yes |

## Click sequence
### Click 1: "Select attacker"
- Pathname before: /immune
- New console: clean
- Network failures: none
- Visible change: dropdown opened; only option is placeholder "Select attacker" (empty in demo mode)
- Pathname after: /immune
- Reverted: n/a

### Click 2: "Select defender"
- Pathname before: /immune
- New console: clean
- Network failures: none
- Visible change: dropdown opened; only option is placeholder "Select defender" (empty in demo mode)
- Pathname after: /immune
- Reverted: n/a

### Click 3: "[number input]"
- Pathname before: /immune
- New console: clean
- Network failures: none
- Visible change: input focused, current value "10"
- Pathname after: /immune
- Reverted: n/a

### Click 4: "Start New Session" (disabled)
- Pathname before: /immune
- New console: clean
- Network failures: none
- Visible change: none (button is disabled — no attacker/defender selected)
- Pathname after: /immune
- Reverted: n/a

### Click 5: "Run Full Scan"
- Pathname before: /immune
- New console: clean
- Network failures: none
- Visible change: none — silent no-op in demo mode
- Pathname after: /immune
- Reverted: n/a

### Click 6: "Enabled" (PII Redaction toggle)
- Pathname before: /immune
- New console: clean
- Network failures: none
- Visible change: none — text remains "Enabled", no state toggle occurred
- Pathname after: /immune
- Reverted: n/a

### Click 7: "Enabled" (API Key Leak Prevention toggle)
- Pathname before: /immune
- New console: clean
- Network failures: none
- Visible change: none — text remains "Enabled", no state toggle occurred
- Pathname after: /immune
- Reverted: n/a

### Click 8: "Disabled" (IP Address Masking toggle)
- Pathname before: /immune
- New console: clean
- Network failures: none
- Visible change: none — text remains "Disabled", no state toggle occurred
- Pathname after: /immune
- Reverted: n/a

### Click 9: "Enabled" (Exfiltration Detection toggle)
- Pathname before: /immune
- New console: clean
- Network failures: none
- Visible change: none — text remains "Enabled", no state toggle occurred
- Pathname after: /immune
- Reverted: n/a

### Click 10: "Save Privacy Rules"
- Pathname before: /immune
- New console: clean
- Network failures: none
- Visible change: none — silent no-op in demo mode
- Pathname after: /immune
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 10
### Elements clicked: 10

## Accessibility
- Images without alt: 0
- Inputs without label: 3 (selectors: `select` "Select attacker" [no id, no label, no aria-label], `select` "Select defender" [no id, no label, no aria-label], `input[type="number"]` [no id, no label, no aria-label])
- Buttons without accessible name: 0
- Sections without aria-label in main: 7 (Live Threat Feed, Live Threat Feed [duplicate heading], Antibody Registry, Immune Memory, Adversarial Arena, Privacy Scanner, Privacy Rules)

## Findings

### immune-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` scrollWidth=2039 > clientWidth=1888 (+151px). Fixed-position element wider than viewport.
- IMPACT: Background element extends beyond viewport, may cause horizontal scroll on some browsers/configurations.

### immune-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=1716 > clientWidth=1577 (+139px). Panel content exceeds its container.
- IMPACT: Content within the main holo-panel overflows its container by 139px. Currently masked by parent overflow rules but indicates layout sizing issue.

### immune-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `h1.nexus-display.m-0` with text "Immune System" is in `div.nexus-control-bar` inside `header`, not inside `main` landmark. DOM path: h1 < div.flex.flex-wrap < div.min-w-[280px] < div.flex.flex-wrap < div.nexus-control-bar.
- IMPACT: Screen readers navigating by landmark will not find the page heading inside the main content region.

### immune-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 7 `<section>` elements inside `main` have no `aria-label` or `aria-labelledby` attribute. Sections: Live Threat Feed, Live Threat Feed (duplicate), Antibody Registry, Immune Memory, Adversarial Arena, Privacy Scanner, Privacy Rules.
- IMPACT: Screen readers cannot distinguish between sections; users navigating by landmark hear "section" with no descriptive name.

### immune-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 3 form inputs lack associated labels: 2 `<select>` elements (attacker/defender) have no `id`, no `<label>`, no `aria-label`; 1 `<input type="number">` (rounds) has no `id`, no `<label>`, no `aria-label`.
- IMPACT: Screen readers cannot announce the purpose of these form controls; WCAG 1.3.1 and 4.1.2 violations.

### immune-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 4 toggle buttons (PII Redaction, API Key Leak Prevention, IP Address Masking, Exfiltration Detection) lack `role="switch"` or `aria-pressed` attributes. Their text label is just "Enabled" or "Disabled" with no indication of which rule they control.
- IMPACT: Screen readers cannot convey toggle state or purpose. Users hear only "Enabled, button" with no context for which setting it controls.

### immune-07
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 4 privacy toggle buttons (clicks 6-9) are no-ops — clicking does not change the text from "Enabled" to "Disabled" or vice versa. `textBefore === textAfter` for all toggles. No console output, no network request, no visible state change.
- IMPACT: Privacy rule toggles appear interactive but do nothing, giving users false confidence that settings can be changed.

### immune-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Run Full Scan" button (click 5) produces no visible change, no console output, and no network request. Silent no-op in demo mode.
- IMPACT: User clicks a prominent action button with no feedback indicating the action was attempted or that demo mode prevents it.

### immune-09
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Save Privacy Rules" button (click 10) produces no visible change, no console output, and no network request. Silent no-op in demo mode.
- IMPACT: User clicks save with no feedback — unclear whether rules were saved or whether the action is unsupported in demo mode.

### immune-10
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Both `<select>` dropdowns ("Select attacker", "Select defender") contain only their placeholder option. No agents are populated in demo mode, making the Adversarial Arena section entirely non-functional.
- IMPACT: The Adversarial Arena feature cannot be explored in demo mode — "Start New Session" is permanently disabled with no explanation.

### immune-11
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: `document.title` is "NexusOS Desktop" — generic, not page-specific (should be "Immune System | NexusOS" or similar).
- IMPACT: Browser tabs, bookmarks, and screen reader title announcements don't identify the current page.

### immune-12
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons in header control bar have no `type` attribute (default to `type="submit"`).
- IMPACT: Buttons default to submit behavior; if wrapped in a form, could trigger unintended form submission.

## Summary
- Gate detected: no
- Total interactive elements: 10
- Elements clicked: 10
- P0: 0
- P1: 1
- P2: 11
