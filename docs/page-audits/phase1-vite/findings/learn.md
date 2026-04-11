# Audit: Learn
URL: http://localhost:1420/learn
Audited at: 2026-04-10T01:20:00Z
Gate detected: false
Gate type: none

## Console (captured at 1248x671, ALL messages)
### Errors
none

### Warnings
1. `[LearningCenter] Error: desktop runtime unavailable` — LearningCenter.tsx:45:37 (via saveProgress -> learningSaveProgress -> invokeDesktop at backend.ts:17)
2. `[LearningCenter] Error: desktop runtime unavailable` — LearningCenter.tsx:45:37 (duplicate from React StrictMode double-invoke of useEffect)

### Logs
none

### Info
1. `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js:21550:24

### Debug
1. `[vite] connecting...` — @vite/client:494:8
2. `[vite] connected.` — @vite/client:617:14

## Overflow

### 1248x671 (actual viewport; resize_window to 1920x1080 had no effect)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]

### 1920x1080
Not measured — resize_window MCP tool reports success but window.innerWidth remains 1248 (known limitation, consistent with prior audits)

### 1280x800
Not measured — same limitation

### 1024x768
Not measured — same limitation

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Courses | button | — | true |
| 2 | Challenges | button | — | true |
| 3 | Build | button | — | true |
| 4 | Knowledge | button | — | true |
| 5 | Progress | button | — | true |
| 6 | (category filter) | select | — | true |

## Click sequence
### Click 1: "Courses"
- Pathname before: /learn
- New console: clean
- Network failures: none
- Visible change: none (already active tab; shows "Courses" view with 5 courses, filter select)
- Pathname after: /learn
- Reverted: n/a

### Click 2: "Challenges"
- Pathname before: /learn
- New console: clean
- Network failures: none
- Visible change: main content switches to "Code Challenges" view showing 0/5 solved, challenge cards (Implement Fuel Check, Capability Gate, Hash Chain Audit, etc.)
- Pathname after: /learn
- Reverted: n/a

### Click 3: "Build"
- Pathname before: /learn
- New console: clean
- Network failures: none
- Visible change: main content switches to "Learn by Building" view showing 4 projects with teach-mode descriptions
- Pathname after: /learn
- Reverted: n/a

### Click 4: "Knowledge"
- Pathname before: /learn
- New console: clean
- Network failures: none
- Visible change: main content switches to "Knowledge Base" view showing 5 articles (e.g., "Why `unsafe` is Forbidden")
- Pathname after: /learn
- Reverted: n/a

### Click 5: "Progress"
- Pathname before: /learn
- New console: clean
- Network failures: none
- Visible change: main content switches to "Your Learning Progress" view showing XP 0, Level 1 Apprentice, course completion bars
- Pathname after: /learn
- Reverted: n/a

### Click 6: select filter -> "Getting Started"
- Pathname before: /learn
- New console: clean
- Network failures: none
- Visible change: course list filters from "5 courses" to "1 courses" (grammar bug: should be "1 course")
- Pathname after: /learn
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 6
### Elements clicked: 6

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `select.lc-filter-select` — no id, no name, no aria-label, no associated `<label>`)
- Buttons without accessible name: 0
- Tab buttons missing ARIA: 5 — all `button.lc-view-btn` elements lack `role="tab"`, `aria-selected`, `aria-controls`, and `type` attribute
- No `role="banner"` landmark on page
- Shell header buttons ("Refresh", "Start Jarvis") also missing `type` attribute (outside main, noted for completeness)

## Findings

### learn-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248 (all measured)
- EVIDENCE: `div.living-background` scrollWidth=1348 > clientWidth=1248, overflow of 100px. `section.holo-panel.holo-panel-mid` scrollWidth=1019 > clientWidth=937, overflow of 82px. Both have `overflow: hidden` so no visible scrollbar, but content is clipped.
- IMPACT: Background and panel content extends beyond viewport; may cause layout shifts or clipping on smaller screens.

### learn-02
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `select.lc-filter-select` has no `id`, no `name`, no `aria-label`, no `aria-labelledby`, and no associated `<label>` element. Screen readers cannot identify the purpose of this control.
- IMPACT: Category filter select is inaccessible to assistive technology users.

### learn-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Five `button.lc-view-btn` elements (Courses, Challenges, Build, Knowledge, Progress) function as tabs but have no `role="tab"`, no `aria-selected`, no `aria-controls`, and no `type` attribute. There is no parent `role="tablist"` container.
- IMPACT: Tab navigation pattern is invisible to screen readers; tab state not communicated.

### learn-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<h1>Learning Center</h1>` is in `header.nexus-shell-header` (outside `<main>`). Inside `<main>`, `<h2 class="lc-sidebar-title">Learning Center</h2>` duplicates the title. The H1 is not inside `<main>`, breaking heading hierarchy for the main content region.
- IMPACT: Screen readers see the primary heading outside the main landmark; duplicate title is redundant.

### learn-05
- SEVERITY: P2
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: Two console warnings on load: `[LearningCenter] Error: desktop runtime unavailable` at LearningCenter.tsx:45:37. The `saveProgress` effect fires on mount and fails because Tauri backend is not available.
- IMPACT: Non-blocking in demo mode but saveProgress call fires unconditionally; may mask real errors.

### learn-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Shell header buttons "Refresh" and "Start Jarvis" lack `type` attribute. Five main-content tab buttons also lack `type` attribute (7 buttons total on page without `type`).
- IMPACT: Buttons default to `type="submit"` per HTML spec; inside a `<form>` this could cause unintended form submission.

### learn-07
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: When category filter selects a single course, the count reads "1 courses" instead of "1 course".
- IMPACT: Minor grammar bug in course count pluralization.

### learn-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: No element on the page has `role="banner"`. The shell header (`header.nexus-shell-header`) lacks this landmark role.
- IMPACT: Assistive technology cannot identify the page banner region.

## Summary
- Gate detected: no
- Total interactive elements: 6
- Elements clicked: 6
- P0: 0
- P1: 0
- P2: 8
