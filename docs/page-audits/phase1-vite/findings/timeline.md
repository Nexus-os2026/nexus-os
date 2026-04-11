# Audit: Timeline
URL: http://localhost:1420/timeline
Audited at: 2026-04-09T21:18:00Z
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

### 1920x1080
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px] (masked by overflow:hidden)

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1346 clientWidth=1248 [OVERFLOW +98px]
  - `section.holo-panel.nexus-page-panel`: scrollWidth=1107 clientWidth=937 [OVERFLOW +170px] (masked by overflow:hidden)

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1066 clientWidth=992 [OVERFLOW +74px]
  - `section.holo-panel.nexus-page-panel`: scrollWidth=826 clientWidth=681 [OVERFLOW +145px] (masked by overflow:hidden)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (type="button") | — | true |
| 2 | All Agents | select | — | true |
| 3 | All Types | select | — | true |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /timeline
- New console: clean
- Network failures: none
- Visible change: none — page remains at "0 events shown" with empty-state message "No audit events yet. Start an agent to generate events."
- Pathname after: /timeline
- Reverted: n/a

### Click 2: "All Agents" (select — change value)
- Pathname before: /timeline
- New console: clean
- Network failures: none
- Visible change: none — select has only 1 option ("All Agents"), no other agents available in demo mode. Cannot change value.
- Pathname after: /timeline
- Reverted: n/a

### Click 3: "All Types" (select — changed to "StateChange")
- Pathname before: /timeline
- New console: clean
- Network failures: none
- Visible change: select value changed to "StateChange". Page still shows "0 events shown" and empty-state message unchanged.
- Pathname after: /timeline
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 3
### Elements clicked: 3 (of 3)

## Accessibility
- Images without alt: 0
- Inputs without label: 2 (selectors: `select.at-select` [agent filter], `select.at-select` [type filter] — both have no id, no aria-label, no aria-labelledby, no label[for], no title)
- Buttons without accessible name: 0

## Findings

### timeline-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.nexus-page-panel` has scrollWidth > clientWidth at all viewports (1920: +139px, 1280: +170px, 1024: +145px). Root cause is `.holo-panel__refraction` decorative element at 2208px width. Overflow is masked by `overflow:hidden` on the section, so no user-visible scrollbar appears, but internal content is clipped.
- IMPACT: Decorative refraction element oversized; currently hidden but may cause layout issues if overflow rule changes.

### timeline-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` has scrollWidth > clientWidth at all viewports (1920: +151px, 1280: +98px, 1024: +74px). No visible scrollbar due to document-level containment but background element extends beyond viewport.
- IMPACT: Background decorative element wider than viewport; cosmetic, no functional impact.

### timeline-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Both `<select class="at-select">` elements (agent filter and type filter) have no `id`, no `aria-label`, no `aria-labelledby`, no `<label for="">`, and no `title` attribute. Screen readers cannot identify what these dropdowns control.
- IMPACT: Filter dropdowns are inaccessible to screen reader users; WCAG 2.1 Level A violation (1.3.1 Info and Relationships, 4.1.2 Name Role Value).

### timeline-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Page has two `<header>` landmark elements: `header.nexus-shell-header` (shell-level) and `header.at-header` (page-level inside section). Neither has an `aria-label` to disambiguate. Screen readers will announce "banner" twice with no way to distinguish them.
- IMPACT: Ambiguous landmark navigation for assistive technology users.

### timeline-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Agent filter `<select>` has only 1 option ("All Agents") in demo mode. The control is rendered and enabled but offers no selectable alternatives, making it a non-functional decoration.
- IMPACT: Agent filter provides no utility in demo mode; user sees a dropdown that cannot change state.

## Summary
- Gate detected: no
- Total interactive elements: 3
- Elements clicked: 3
- P0: 0
- P1: 0
- P2: 5
