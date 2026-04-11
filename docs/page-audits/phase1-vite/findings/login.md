# Audit: Login
URL: http://localhost:1420/login
Audited at: 2026-04-10T00:18:29Z
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

> **Note:** `resize_window` did not change the JavaScript-reported viewport dimensions (remained 1888x951 across all three requested sizes). Measurements below are all at the actual viewport of 1888x951.

### 1920x1080 (actual 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each, sidebar text clipping]

### 1280x800 (actual 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

### 1024x768 (actual 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | yes |
| 2 | Start Jarvis | button | — | yes |
| 3 | Logout | button | — | yes |

## Click sequence

### Click 1: "Refresh"
- Pathname before: /login
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /login
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /login
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /login
- Reverted: n/a

### Skipped (destructive)
- "Logout" — reason: destructive keyword "logout"

### Total interactive elements found: 3
### Elements clicked: 2 (1 skipped as destructive)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### login-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` scrollWidth=2039 exceeds clientWidth=1888 by 151px. This is the animated background layer that overflows the viewport on every measured size.
- IMPACT: Horizontal overflow creates a hidden scrollbar or clipped content depending on parent overflow setting.

### login-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=1716 exceeds clientWidth=1577 by 139px.
- IMPACT: The main page panel content area overflows its container, potentially clipping content or causing layout shifts.

### login-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two H1 elements on the page: "Auth / Sessions" (in `div.flex` page header) and "Session & Auth" (in `div.admin-shell` main content). HTML spec recommends a single H1 per page.
- IMPACT: Screen readers announce two top-level headings, confusing document structure for assistive technology users.

### login-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 3 buttons in main content ("Refresh", "Start Jarvis", "Logout") have `type` attribute = null. Without explicit `type="button"`, browsers default to `type="submit"` which can trigger unintended form submission.
- IMPACT: If any of these buttons are placed inside a `<form>` in the future, they will submit the form on click instead of performing their intended action.

### login-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button click produces no console output, no navigation, no network request, and no visible change.
- IMPACT: Button appears functional but is a silent no-op in demo mode; no user feedback that the action cannot be performed.

### login-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button click produces no console output, no navigation, no network request, and no visible change.
- IMPACT: Button appears functional but is a silent no-op in demo mode; no user feedback that the action cannot be performed.

### login-07
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: "Current Session" card shows empty values for Name (empty string), Email (empty string), Session ID (empty string), and Issuer URL (empty string). The `<span>` siblings exist but contain no text.
- IMPACT: Session data fields render as blank, giving no indication to the user whether data is missing, loading, or intentionally empty. A placeholder like "—" or "N/A" would clarify intent.

### login-08
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: Page header title is "Auth / Sessions" but the main content heading is "Session & Auth". The sidebar nav item is "Auth / Sessions".
- IMPACT: Inconsistent naming between header and content area creates confusion about which page the user is on.

## Summary
- Gate detected: no
- Total interactive elements: 3
- Elements clicked: 2
- P0: 0
- P1: 2
- P2: 6
