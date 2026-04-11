# Audit: Time Machine
URL: http://localhost:1420/time-machine
Audited at: 2026-04-09T23:49:00Z
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
- `Download the React DevTools for a better development experience: https://reactjs.org/link/react-devtools` — chunk-NUMECXU6.js?v=5144749d:21550:24

### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080
(actual viewport 1888x951 — browser chrome prevents exact 1920x1080)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px, sidebar text]

### 1280x800
(viewport locked at 1888x951 — resize_window has no effect; measurements identical to above)

### 1024x768
(viewport locked at 1888x951 — resize_window has no effect; measurements identical to above)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh (header) | button | — | yes |
| 2 | Start Jarvis (header) | button | — | yes |
| 3 | Undo | button | — | no (disabled) |
| 4 | Redo | button | — | yes |
| 5 | + Checkpoint | button | — | yes |
| 6 | Replay & Evidence | button (tab) | — | yes |
| 7 | Temporal History | button (tab) | — | yes |
| 8 | Start Recording | button | — | yes |
| 9 | Filter by agent ID... | input[text] | — | yes |
| 10 | Refresh (main) | button | — | yes |

## Click sequence
### Click 1: "Refresh" (header, ref_217)
- Pathname before: /time-machine
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /time-machine
- Reverted: n/a

### Click 2: "Start Jarvis" (header, ref_218)
- Pathname before: /time-machine
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /time-machine
- Reverted: n/a

### Click 3: "Redo" (ref_226)
- Pathname before: /time-machine
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /time-machine
- Reverted: n/a

### Click 4: "+ Checkpoint" (ref_227)
- Pathname before: /time-machine
- New console: clean
- Network failures: none
- Visible change: none — checkpoint count stays "0 checkpoints", silent no-op
- Pathname after: /time-machine
- Reverted: n/a

### Click 5: "Replay & Evidence" (ref_231)
- Pathname before: /time-machine
- New console: clean
- Network failures: none
- Visible change: none — already the active tab (green border-bottom, green text)
- Pathname after: /time-machine
- Reverted: n/a

### Click 6: "Temporal History" (ref_232)
- Pathname before: /time-machine
- New console: clean
- Network failures: none
- Visible change: none — tab content does not switch, "No replay bundles found." still displayed, "Error: desktop runtime unavailable" still visible
- Pathname after: /time-machine
- Reverted: n/a

### Click 7: "Start Recording" (ref_233)
- Pathname before: /time-machine
- New console: clean
- Network failures: none
- Visible change: none — button text stays "Start Recording", silent no-op
- Pathname after: /time-machine
- Reverted: n/a

### Click 8: "Filter by agent ID..." (ref_234)
- Pathname before: /time-machine
- New console: clean
- Network failures: none
- Visible change: input receives click (focus expected)
- Pathname after: /time-machine
- Reverted: n/a

### Click 9: "Refresh" (main, ref_235)
- Pathname before: /time-machine
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /time-machine
- Reverted: n/a

### Skipped (disabled)
- "Undo" — reason: disabled (button.disabled=true)

### Skipped (destructive)
none

### Total interactive elements found: 10
### Elements clicked: 9 (1 skipped: disabled)

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `input[type="text"][placeholder="Filter by agent ID..."]`)
- Buttons without accessible name: 0

## Findings

### time-machine-01
- SEVERITY: P1
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Error: desktop runtime unavailable" is rendered as a visible `<div>` (display:block, visibility:visible, opacity:1, offsetHeight=30px) inside the main content area at ref_236. Text is user-facing.
- IMPACT: Users see a raw error string with no explanation, no call-to-action, and no styling to indicate it is an expected limitation of demo mode.

### time-machine-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `div.living-background` scrollWidth=2039 > clientWidth=1888 (+151px overflow). Background element extends beyond viewport.
- IMPACT: Cosmetic — may cause horizontal scrollbar on some browsers or clip background animation.

### time-machine-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=1716 > clientWidth=1577 (+139px overflow). Panel content overflows its container.
- IMPACT: Content may be clipped or cause layout shift; overflow:hidden on parent masks the issue.

### time-machine-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Duplicate H1 elements — `<h1>Time Machine</h1>` appears twice: once in the banner/header (outside `<main>`) at ref_211, and once inside `<main>` at ref_222. Two H1 elements on one page violates heading hierarchy best practices.
- IMPACT: Screen readers announce two top-level headings, confusing document outline.

### time-machine-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: H1 "Time Machine" at ref_211 is outside the `<main>` element, placed inside the `[role="banner"]` header. The `<main>` landmark should contain the primary heading.
- IMPACT: Landmark navigation may skip the page title when users jump to `<main>`.

### time-machine-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `input[type="text"][placeholder="Filter by agent ID..."]` at ref_234 has no `<label>`, no `aria-label`, and no `aria-labelledby`. Only a placeholder attribute provides context.
- IMPACT: Screen readers cannot announce the input's purpose; placeholder disappears on focus.

### time-machine-07
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Temporal History" tab button (ref_232) does not switch content. After clicking it, the "Replay & Evidence" tab retains its active visual state (green border-bottom, green text color) and content remains "No replay bundles found." / "Error: desktop runtime unavailable". The tab appears non-functional.
- IMPACT: Users cannot access the Temporal History view; tab UI implies functionality that does not exist.

### time-machine-08
- SEVERITY: P1
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: All 7 enabled buttons in the main content area are silent no-ops in demo mode: "Redo", "+ Checkpoint", "Replay & Evidence", "Temporal History", "Start Recording", "Refresh" (main), plus header "Refresh" and "Start Jarvis". No console output, no error messages, no visual feedback of any kind on click.
- IMPACT: Users get zero feedback when interacting with the page; no indication that actions require a backend.

### time-machine-09
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The banner/header element at ref_209 has `role="banner"` but no `aria-label`. The sidebar navigation at ref_5 has `<nav>` with no `aria-label`.
- IMPACT: When multiple landmarks exist, unlabeled landmarks cannot be distinguished by assistive technology.

## Summary
- Gate detected: no
- Total interactive elements: 10
- Elements clicked: 9
- P0: 0
- P1: 3
- P2: 6
