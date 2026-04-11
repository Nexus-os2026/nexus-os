# Audit: Admin Health
URL: http://localhost:1420/admin-health
Audited at: 2026-04-10T00:53:00Z
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

NOTE: Chrome extension viewport is locked at 992x639 regardless of window.resizeTo(). All three requested sizes report identical measurements. Overflow data is from the effective 992x639 viewport.

### 1920x1080 (effective 992x639)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `MAIN.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `DIV.living-background`: scrollWidth=1071 clientWidth=992 [OVERFLOW +79px]
  - `SECTION.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=741 clientWidth=681 [OVERFLOW +60px]
  - `SPAN.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]

### 1280x800 (effective 992x639)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `MAIN.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing: same as above

### 1024x768 (effective 992x639)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `MAIN.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing: same as above

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (in header, outside main) | n/a | yes |
| 2 | Start Jarvis | button (in header, outside main) | n/a | yes |
| 3 | Create Backup | button (in main) | n/a | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /admin-health
- New console: clean
- Network failures: none
- Visible change: none (silent no-op)
- Pathname after: /admin-health
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /admin-health
- New console: clean
- Network failures: none
- Visible change: none (silent no-op)
- Pathname after: /admin-health
- Reverted: n/a

### Click 3: "Create Backup"
- Pathname before: /admin-health
- New console: clean
- Network failures: none
- Visible change: none (silent no-op)
- Pathname after: /admin-health
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 3
### Elements clicked: 3

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Buttons without explicit `type` attribute: 3 (`Refresh`, `Start Jarvis`, `Create Backup`)

## Findings

### admin-health-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `DIV.living-background` scrollWidth=1071 > clientWidth=992 (+79px). This is a persistent layout-level overflow on the background container.
- IMPACT: Horizontal scroll may appear or content may be clipped on narrower viewports; background bleeds past document edge.

### admin-health-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `SECTION.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=741 > clientWidth=681 (+60px). The main page panel overflows its container by 60px.
- IMPACT: Panel content may be clipped or cause unexpected horizontal scroll within the content area.

### admin-health-03
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page: "Admin Health" (in `<header>`, outside `<main>`) and "System Health" (inside `<main>`). Dual H1 violates WCAG heading hierarchy.
- IMPACT: Screen readers announce two document titles, confusing landmark navigation.

### admin-health-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Three buttons lack explicit `type` attribute: `Refresh`, `Start Jarvis`, `Create Backup`. Default type is "submit" which can trigger unintended form submissions.
- IMPACT: Implicit submit type may cause unexpected behavior if buttons are inside or near a `<form>` element.

### admin-health-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 3 interactive buttons ("Refresh", "Start Jarvis", "Create Backup") produce no console output, no network requests, and no visible UI change when clicked in demo mode. All are silent no-ops.
- IMPACT: User clicks buttons expecting feedback but receives none; no loading indicator, toast, or disabled-state change.

### admin-health-06
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: "Last Backup" and "Next Scheduled" display identical timestamps: "4/10/2026, 12:51:12 AM". In demo mode the mock data sets both to the current time.
- IMPACT: Misleading — suggests the next backup is already overdue or that scheduling is broken. Demo data should show a future timestamp for "Next Scheduled".

### admin-health-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Instances table (columns: Hostname, Status, CPU, Memory, Disk, Agents, Uptime) has `<thead>` but `<tbody>` contains 0 rows. No empty-state message displayed. LLM Providers table (columns: Provider, Status, Latency, Error Rate, Requests 24h) also has 0 `<tbody>` rows with no empty-state message.
- IMPACT: User sees table headers with no data and no explanation; appears broken rather than intentionally empty.

### admin-health-08
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: Three `SPAN.nexus-sidebar-item-text` elements overflow by 4px (scrollWidth=157, clientWidth=153). Sidebar text is clipped.
- IMPACT: Long sidebar item labels are truncated without ellipsis or tooltip; minor cosmetic issue in sidebar.

## Summary
- Gate detected: no
- Total interactive elements: 3
- Elements clicked: 3
- P0: 0
- P1: 2
- P2: 6
