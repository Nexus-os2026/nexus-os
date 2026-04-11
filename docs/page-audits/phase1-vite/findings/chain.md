# Audit: Chain
URL: http://localhost:1420/chain
Audited at: 2026-04-10T01:10:00Z
Gate detected: false
Gate type: none

## Console (captured at 1248x671 — viewport locked, see note)
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

> **Note:** Viewport resize via MCP `resize_window` had no effect — window remained locked at 1248x671 (ultrawide 3440x1440 display). All three viewports report identical measurements collected at 1248x671.

### 1920x1080 (measured at 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [minor text overflow]

### 1280x800 (measured at 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing: same as above

### 1024x768 (measured at 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing: same as above

## Interactive elements (main content only)

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| (none) | — | — | — | — |

Zero interactive elements found inside `<main>`. The shell header (outside `<main>`) contains 2 buttons:
- "Refresh" — `button` (no `type` attribute), enabled
- "Start Jarvis" — `button` (no `type` attribute), enabled

## Click sequence

No clicks performed — zero interactive elements in main content area.

### Skipped (destructive)
none

### Total interactive elements found: 0
### Elements clicked: 0 (nothing to click)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

Additional observations:
- H1 "Distributed Audit" is outside `<main>` (in shell header `header.nexus-shell-header`). Main content starts with H2 "DISTRIBUTED AUDIT // IMMUTABLE CHAIN".
- Two `<header>` elements exist: `header.nexus-shell-header` (parent: `DIV`) and `header.da-header` (parent: `SECTION`). Neither is a direct child of `<body>`, so neither receives implicit `banner` landmark role.
- No element has explicit `role="banner"`.
- Shell header buttons "Refresh" and "Start Jarvis" both lack `type` attribute.

## Findings

### chain-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all (measured at 1248)
- EVIDENCE: `div.living-background` scrollWidth=1348 exceeds clientWidth=1248 by 100px.
- IMPACT: Decorative background element overflows viewport width; hidden by body `overflow:hidden` but generates unnecessary layout.

### chain-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all (measured at 1248)
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=1019 exceeds clientWidth=937 by 82px.
- IMPACT: Main content panel overflows its container; may cause layout issues at narrower viewports.

### chain-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Single `<h1>` "Distributed Audit" is in `header.nexus-shell-header`, outside `<main>`. Main content starts with `<h2>`. Only 1 H1 on page (no duplicate).
- IMPACT: Screen readers entering main content find no H1 landmark; heading hierarchy starts at H2 inside main.

### chain-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<header>` elements (`header.nexus-shell-header` parent=DIV, `header.da-header` parent=SECTION). Neither is a direct child of `<body>`. Zero elements have `role="banner"`. Zero implicit banner landmarks.
- IMPACT: Page has no banner landmark for assistive technology navigation.

### chain-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Shell header buttons "Refresh" and "Start Jarvis" in `header.nexus-shell-header` lack `type` attribute. Both default to `type="submit"`.
- IMPACT: Buttons may trigger unintended form submission if wrapped in a `<form>` ancestor.

### chain-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: `document.querySelector('main').querySelectorAll('button, input, select, textarea, a[href]')` returns empty NodeList. Main content contains only static text: headings, stats cards ("Blocks", "Events", "Devices", "1/1", "Synced", "CLEAN", "Tamper"), empty audit chain ("No events / Chain is empty"), and one paired device card ("nexus-primary").
- IMPACT: Page is entirely read-only with no user actions available in the main content area; users cannot interact with the audit chain, filter events, or manage devices.

## Summary
- Gate detected: no
- Total interactive elements: 0
- Elements clicked: 0
- P0: 0
- P1: 0
- P2: 6
