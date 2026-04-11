# Audit: Software Factory
URL: http://localhost:1420/software-factory
Audited at: 2026-04-09T20:39:00Z
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
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js:21550:24

### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `.nexus-main-column`: scrollWidth=1630 clientWidth=1630 [OK]
- `section.holo-panel`: scrollWidth=1129 clientWidth=937 [OVERFLOW — 192px clipped by overflow:hidden]
- `.holo-panel-mid`: scrollWidth=1191 clientWidth=937 [OVERFLOW — 254px clipped by overflow:hidden]
- `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW — 151px, fixed-position decorative layer, clipped by overflow:hidden]

### 1280x800
- Note: Browser window could not be resized below inner viewport 1888px (maximized/constrained). Layout identical to 1920x1080.
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `.nexus-main-column`: scrollWidth=1630 clientWidth=1630 [OK]
- `section.holo-panel`: scrollWidth=1129 clientWidth=937 [OVERFLOW — clipped]
- `.holo-panel-mid`: scrollWidth=1191 clientWidth=937 [OVERFLOW — clipped]

### 1024x768
- Note: Same constraint as 1280x800. Layout identical to 1920x1080.
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `.nexus-main-column`: scrollWidth=1630 clientWidth=1630 [OK]
- `section.holo-panel`: scrollWidth=1129 clientWidth=937 [OVERFLOW — clipped]
- `.holo-panel-mid`: scrollWidth=1191 clientWidth=937 [OVERFLOW — clipped]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | yes |
| 2 | Start Jarvis | button | — | yes |
| 3 | (placeholder: "Project title") | input | — | yes |
| 4 | (placeholder: "Describe what to build...") | textarea | — | yes |
| 5 | Create Project | button | — | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /software-factory
- New console: clean
- Network failures: none
- Visible change: none — button click produces no observable effect
- Pathname after: /software-factory
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /software-factory
- New console: clean
- Network failures: none
- Visible change: none — button click produces no observable effect
- Pathname after: /software-factory
- Reverted: n/a

### Click 3: Input "Project title"
- Pathname before: /software-factory
- New console: clean
- Network failures: none
- Visible change: input receives click but `document.activeElement` does not match element (focus not confirmed)
- Pathname after: /software-factory
- Reverted: n/a

### Click 4: Textarea "Describe what to build..."
- Pathname before: /software-factory
- New console: clean
- Network failures: none
- Visible change: textarea receives click but `document.activeElement` does not match element (focus not confirmed)
- Pathname after: /software-factory
- Reverted: n/a

### Click 5: "Create Project"
- Pathname before: /software-factory
- New console: clean
- Network failures: none
- Visible change: none — accepts empty submission with no validation feedback
- Pathname after: /software-factory
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 5
### Elements clicked: 5

## Accessibility
- Images without alt: 0
- Inputs without label: 2 (selectors: `input[type="text"]` with placeholder "Project title", `textarea` with placeholder "Describe what to build...")
- Buttons without accessible name: 0
- Label elements on page: 0
- Duplicate H1: 2 — both read "Software Factory" (one in header `.flex.flex-wrap.items-center.gap-2.5`, one in `.holo-panel__content`)
- ARIA landmarks: ASIDE, NAV, HEADER, MAIN present
- Header elements: 1

## Findings

### software-factory-01
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements both containing "Software Factory". First H1 is in the page header bar (parent `.flex.flex-wrap.items-center.gap-2.5`), second H1 is inside `.holo-panel__content`. Both are visible.
- IMPACT: Duplicate H1 elements violate WCAG heading hierarchy; screen readers cannot determine the primary page heading.

### software-factory-02
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<input>` with placeholder "Project title" has no `<label>`, no `aria-label`, no `id` for label association. `<textarea>` with placeholder "Describe what to build..." has no `<label>`, no `aria-label`, no `id`. Zero `<label>` elements on entire page.
- IMPACT: Screen readers announce these as unlabeled form controls; placeholder text disappears on input and is not a substitute for a label.

### software-factory-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All three buttons (`Refresh`, `Start Jarvis`, `Create Project`) have no `type` attribute. None are inside a `<form>`. Per HTML spec, buttons without `type` default to `type="submit"`. No `<form>` element wraps the input/textarea/button group.
- IMPACT: Without `type="button"`, buttons default to submit behavior. The missing form wrapper means the input group has no semantic form structure for assistive tech or native validation.

### software-factory-04
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button click produces no visible change, no console output, no network request. "Start Jarvis" button click produces no visible change, no console output, no network request. Both fail silently.
- IMPACT: User-facing controls that do nothing provide a broken interaction — users cannot refresh data or start the Jarvis agent.

### software-factory-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Create Project" button clicked with both input and textarea empty. No validation message, no error, no console output. The form accepts empty submission silently.
- IMPACT: Users get no feedback that a project name or description is required; the action appears broken.

### software-factory-06
- SEVERITY: P1
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: Visible text reads "Budget: Error: desktop runtime unavailable" in a `<div>` inside the main content area. The raw error string is rendered as user-facing copy.
- IMPACT: Exposes an internal error message to the user instead of a graceful fallback (e.g., "Budget: unavailable in demo mode").

### software-factory-07
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel` has `overflow:hidden` clipping 192px of content (scrollWidth=1129, clientWidth=937). `.holo-panel-mid` clips 254px (scrollWidth=1191, clientWidth=937). The decorative `.holo-panel__refraction` layer extends beyond the panel boundary.
- IMPACT: Content inside holo-panel may be silently clipped; decorative refraction layer overflows container bounds.

## Summary
- Gate detected: no
- Total interactive elements: 5
- Elements clicked: 5
- P0: 0
- P1: 2
- P2: 5
