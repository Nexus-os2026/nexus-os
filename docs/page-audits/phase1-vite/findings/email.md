# Audit: Email
URL: http://localhost:1420/email
Audited at: 2026-04-09T20:52:00Z
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
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js:21550

### Debug
- `[vite] connecting...` — @vite/client:494
- `[vite] connected.` — @vite/client:617

## Overflow

### 1920x1080
(Actual viewport: 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `.nexus-main-column`: scrollWidth=1630 clientWidth=1630 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `.holo-panel-mid`: scrollWidth=1247 clientWidth=937 [OVERFLOW +310px horizontal, +461px vertical — content clipped]
  - `div.ec-email-list`: scrollWidth=347 clientWidth=319 [OVERFLOW +28px]
  - `div.ec-list-header`: scrollWidth=347 clientWidth=319 [OVERFLOW +28px]
  - `span.nexus-sidebar-item-text` (×3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

### 1280x800
Viewport resize had no effect — window.innerWidth remains 1888. Same overflow values as 1920x1080.

### 1024x768
Viewport resize had no effect — window.innerWidth remains 1888. Same overflow values as 1920x1080.

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | yes |
| 2 | Start Jarvis | button | — | yes |
| 3 | Compose | button | — | yes |
| 4 | Sign in with Google | button | — | yes |
| 5 | Sign in with Microsoft | button | — | yes |
| 6 | Inbox | button | — | yes |
| 7 | Starred | button | — | yes |
| 8 | Sent | button | — | yes |
| 9 | Drafts | button | — | yes |
| 10 | Queued | button | — | yes |
| 11 | Archive | button | — | yes |
| 12 | Trash | button | — | yes |
| 13 | All | button | — | yes |
| 14 | Primary | button | — | yes |
| 15 | Updates | button | — | yes |
| 16 | Social | button | — | yes |
| 17 | Promotions | button | — | yes |
| 18 | Agent | button | — | yes |
| 19 | (placeholder: "Search emails...") | input[text] | — | yes |
| 20 | (options: Date, Priority, Unread First) | select | — | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /email
- New console: clean
- Network failures: none
- Visible change: none observed
- Pathname after: /email
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /email
- New console: clean
- Network failures: none
- Visible change: none observed
- Pathname after: /email
- Reverted: n/a

### Click 3: "Compose"
- Pathname before: /email
- New console: clean
- Network failures: none
- Visible change: none — no compose modal or panel appeared
- Pathname after: /email
- Reverted: n/a

### Click 4: "Sign in with Google"
- Pathname before: /email
- New console: clean
- Network failures: none
- Visible change: none — OAuth failed silently; "OAuth failed: Error: desktop runtime unavailable" already shown in audit section
- Pathname after: /email
- Reverted: n/a

### Click 5: "Sign in with Microsoft"
- Pathname before: /email
- New console: clean
- Network failures: none
- Visible change: none — same as Google sign-in
- Pathname after: /email
- Reverted: n/a

### Click 6: "Inbox"
- Pathname before: /email
- New console: clean
- Network failures: none
- Visible change: Inbox folder gains `active` class
- Pathname after: /email
- Reverted: n/a

### Click 7: "Starred"
- Pathname before: /email
- New console: clean
- Network failures: none
- Visible change: Starred folder gains `active` class, Inbox loses it
- Pathname after: /email
- Reverted: n/a

### Click 8: "Sent"
- Pathname before: /email
- New console: clean
- Network failures: none
- Visible change: Sent folder gains `active` class
- Pathname after: /email
- Reverted: n/a

### Click 9: "Drafts"
- Pathname before: /email
- New console: clean
- Network failures: none
- Visible change: Drafts folder gains `active` class
- Pathname after: /email
- Reverted: n/a

### Click 10: "Queued"
- Pathname before: /email
- New console: clean
- Network failures: none
- Visible change: Queued folder gains `active` class
- Pathname after: /email
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 20
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 2 (selectors: `input.ec-search[type=text]`, `select.ec-sort-select`)
- Buttons without accessible name: 0
- Additional: 0 of 18 buttons in main content have a `type` attribute (all default to `submit`). 0 `<label>` elements on page.

## Findings

### email-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `div.ec-email-list` (scrollWidth=347, clientWidth=319, +28px) and `div.ec-list-header` (scrollWidth=347, clientWidth=319, +28px). Root cause: `.ec-list-header` is `display:flex` with gap=8px and padding=10px. Children `input.ec-search` (187px) + `select.ec-sort-select` (142px) + gap(8px) + padding(20px) = 357px, exceeding 319px container by 28px. `overflow:visible` on `.ec-list-header` propagates to `.ec-email-list` which has `overflow:hidden`, silently clipping the select dropdown.
- IMPACT: Search input and sort select are clipped on the right edge; select control may be partially hidden.

### email-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `.holo-panel-mid` scrollWidth=1247 clientWidth=937 (+310px horizontal), scrollHeight=797 clientHeight=336 (+461px vertical). `overflow:hidden` clips 310px of horizontal content and 461px of vertical content.
- IMPACT: Significant portion of the email client UI is silently clipped and invisible to users at default viewport.

### email-03
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: `.ec-audit-entry` elements render raw error string "OAuth failed: Error: desktop runtime unavailable" (×2) in the Activity section of the email sidebar. These are visible to the user as plain text.
- IMPACT: Raw error strings exposed in UI instead of user-friendly messages; confusing for end users.

### email-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `input.ec-search[type=text]` and `select.ec-sort-select` have no associated `<label>`, `aria-label`, or `aria-labelledby`. Zero `<label>` elements exist on the page. Input relies solely on `placeholder="Search emails..."` for identification.
- IMPACT: Screen readers cannot identify the purpose of these form controls; placeholder text disappears on input.

### email-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 18 buttons in the main content area lack a `type` attribute (0 of 18 have `type`). Without explicit `type="button"`, they default to `type="submit"`, which can cause unintended form submission if wrapped in a `<form>` element.
- IMPACT: Potential unintended form submission behavior; does not follow HTML best practices.

### email-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Clicking "Compose" button (`.ec-btn-compose`) produces no visible change — no compose modal, panel, or navigation. No console output, no network requests. Button appears fully inert in demo mode.
- IMPACT: Primary email action (compose) is non-functional with no feedback to the user.

### email-07
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `div.living-background` scrollWidth=2039, clientWidth=1888 (+151px). Decorative background element extends 151px beyond viewport. Parent clips via `overflow:hidden` on body/html preventing visible scrollbar.
- IMPACT: Cosmetic — no user-visible impact due to parent clipping, but contributes to layout calculation overhead.

### email-08
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=1716, clientWidth=1577 (+139px). The `.holo-panel__refraction` child is absolutely positioned at left=-187px with width=1312px, intentionally oversized for visual effect. Parent `overflow:hidden` clips it.
- IMPACT: Known decorative pattern (consistent with deploy, software-factory, protocols pages). No user-visible scrollbar, but contributes to overflow measurements.

## Summary
- Gate detected: no
- Total interactive elements: 20
- Elements clicked: 10
- P0: 0
- P1: 2
- P2: 6
