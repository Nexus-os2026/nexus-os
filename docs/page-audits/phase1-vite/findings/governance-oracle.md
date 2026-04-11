# Audit: Governance Oracle
URL: http://localhost:1420/governance-oracle
Audited at: 2026-04-09T22:34:00Z
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
1. `Error: desktop runtime unavailable` at `src/api/backend.ts:17:11` via `oracleStatus` at `src/api/backend.ts:2457:10`, triggered from `src/pages/GovernanceOracle.tsx:39:18` (commitPassiveMountOnFiber path)
2. `Error: desktop runtime unavailable` at `src/api/backend.ts:17:11` via `oracleStatus` at `src/api/backend.ts:2457:10`, triggered from `src/pages/GovernanceOracle.tsx:39:18` (React StrictMode double-invoke path via invokePassiveEffectMountInDEV)

### Warnings
none

### Logs
none

### Info
none

### Debug
none

## Overflow

### 1920x1080
Actual viewport: 1888x951

- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]

### 1280x800
Viewport unchanged at 1888x951 (Chrome window resize does not affect inner viewport in this environment).

- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]

### 1024x768
Viewport unchanged at 1888x951.

- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| (none) | — | — | — | — |

No interactive elements found inside `<main>`.

### Banner-level interactive elements (outside main, outside sidebar)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no type attr) | — | true |
| 2 | Start Jarvis | button (no type attr) | — | true |

## Click sequence

### Click 1: "Refresh" (banner)
- Pathname before: /governance-oracle
- New console: clean (no new messages)
- Network failures: none
- Visible change: none — button is completely inert
- Pathname after: /governance-oracle
- Reverted: n/a

### Click 2: "Start Jarvis" (banner)
- Pathname before: /governance-oracle
- New console: clean (no new messages)
- Network failures: none
- Visible change: none — button is completely inert
- Pathname after: /governance-oracle
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 0 (main), 2 (banner)
### Elements clicked: 2

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Duplicate H1 elements: 2 (banner `h1` "Governance Oracle" + main `h1` "Governance Oracle"), both simultaneously visible
- Sections without accessible name: 4 (`section.holo-panel.holo-panel-mid.nexus-page-panel`, 3 unnamed `<section>` elements used as content regions lack `aria-label`/`aria-labelledby`)
- ARIA live regions in main: 0 (status text "Oracle not initialized" is a plain `<div>` with no `aria-live`)

## Findings

### governance-oracle-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: Two `Error: desktop runtime unavailable` thrown from `GovernanceOracle.tsx:39` via `oracleStatus()` at `backend.ts:2457`. The `useEffect` call does not catch the error, letting it propagate to React's passive effect handler. Second error is React StrictMode double-invoke.
- IMPACT: Uncaught errors in effects can cascade; the oracle status section renders "Oracle not initialized" with no graceful fallback messaging tied to demo mode.

### governance-oracle-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all (measured at 1888px)
- EVIDENCE: `div.living-background` has scrollWidth=2039 vs clientWidth=1888 (+151px horizontal overflow). This is a background decoration element that exceeds the viewport.
- IMPACT: May cause horizontal scrollbar on some browsers or clip decorative content unexpectedly.

### governance-oracle-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all (measured at 1888px)
- EVIDENCE: `section.holo-panel.holo-panel-mid` has scrollWidth=1716 vs clientWidth=1577 (+139px horizontal overflow) and `overflow:hidden` set. Vertically, scrollHeight=1039 vs clientHeight=693 — 346px of content clipped silently.
- IMPACT: Content within the holo-panel (Security Properties, Agent Budget Viewer sections) is silently clipped. Users cannot scroll to see potentially hidden content at the bottom.

### governance-oracle-04
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements both containing "Governance Oracle" are rendered simultaneously — one in the page-level banner (`header`) and one inside `<main>`. Both are visible (offsetParent !== null).
- IMPACT: Screen readers announce two identical level-1 headings, violating the single-H1 best practice and creating confusion about document structure.

### governance-oracle-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button in the banner is completely inert in demo mode — no console output, no visible change, no network request, no user feedback.
- IMPACT: Users clicking Refresh get zero feedback, making the button appear broken.

### governance-oracle-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button in the banner is completely inert in demo mode — no console output, no visible change, no network request, no user feedback.
- IMPACT: Users clicking Start Jarvis get zero feedback, making the button appear broken.

### governance-oracle-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Both banner buttons ("Refresh", "Start Jarvis") lack the `type` attribute. In a `<form>` context these would default to `type="submit"`, potentially causing unintended form submissions.
- IMPACT: Missing `type="button"` is a defensive-coding gap; browsers default to `submit` inside forms.

### governance-oracle-08
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Oracle not initialized" status text in `section > div` has no `aria-live` attribute, no `role="status"`, and no `role="alert"`. The three content sections (Oracle Status, Security Properties, Agent Budget Viewer) are `<section>` elements without `aria-label` or `aria-labelledby`, so they lack accessible names for landmark navigation.
- IMPACT: Screen readers will not announce status changes or be able to navigate to named content regions.

## Summary
- Gate detected: no
- Total interactive elements: 0 (main), 2 (banner)
- Elements clicked: 2
- P0: 0
- P1: 3
- P2: 5
