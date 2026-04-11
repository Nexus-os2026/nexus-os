# Audit: Deploy
URL: http://localhost:1420/deploy
Audited at: 2026-04-09T20:35:00+01:00
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
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px, decorative layer]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1770 clientWidth=1577 [OVERFLOW +193px, clipped by overflow:hidden]
  - `div.dp-audit-entry` (x4): scrollWidth=355-554 clientWidth=239 [OVERFLOW +116-315px, clipped by overflow:hidden + text-overflow:ellipsis]

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px, decorative]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1174 clientWidth=937 [OVERFLOW +237px, clipped]
  - `div.dp-audit-entry` (x4): scrollWidth=355-554 clientWidth=224 [OVERFLOW +131-330px, clipped]

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1075 clientWidth=992 [OVERFLOW +83px, decorative]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=823 clientWidth=681 [OVERFLOW +142px, clipped]
  - `div.dp-audit-entry` (x4): scrollWidth=355-554 clientWidth=224 [OVERFLOW +131-330px, clipped]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | + Project | button | — | yes |
| 2 | Projects | button | — | yes |
| 3 | Pipeline | button | — | yes |
| 4 | History | button | — | yes |
| 5 | Logs | button | — | yes |
| 6 | Airgap | button | — | yes |
| 7 | + New Project | button | — | yes |

## Click sequence
### Click 1: "+ Project"
- Pathname before: /deploy
- New console: clean
- Network failures: none
- Visible change: Modal overlay (div.dp-hitl-overlay) appeared with "New Project" form containing Project Name input, Language select (rust/javascript/typescript/python/go), Source Directory input, Create Project and Cancel buttons
- Pathname after: /deploy
- Reverted: n/a

### Click 2: "Projects" (tab)
- Pathname before: /deploy
- New console: clean
- Network failures: none
- Visible change: Projects tab active (already was active), right panel shows Projects (0) with cloud provider list and empty state
- Pathname after: /deploy
- Reverted: n/a

### Click 3: "Pipeline" (tab)
- Pathname before: /deploy
- New console: clean
- Network failures: none
- Visible change: Pipeline tab becomes active, right panel shows "No Deployment Pipelines Configured" empty state
- Pathname after: /deploy
- Reverted: n/a

### Click 4: "History" (tab)
- Pathname before: /deploy
- New console: clean
- Network failures: none
- Visible change: History tab becomes active, right panel shows "No Pipeline History Yet" empty state
- Pathname after: /deploy
- Reverted: n/a

### Click 5: "Logs" (tab)
- Pathname before: /deploy
- New console: clean
- Network failures: none
- Visible change: Logs tab becomes active, panel content updates
- Pathname after: /deploy
- Reverted: n/a

### Click 6: "Airgap" (tab)
- Pathname before: /deploy
- New console: clean
- Network failures: none
- Visible change: Airgap tab becomes active, panel content updates
- Pathname after: /deploy
- Reverted: n/a

### Click 7: "+ New Project"
- Pathname before: /deploy
- New console: clean
- Network failures: none
- Visible change: Same modal overlay as Click 1 — "New Project" form appeared
- Pathname after: /deploy
- Reverted: n/a (modal closed via Cancel)

### Additional click test: "Create Project" (inside modal)
- Clicked with empty Project Name input
- No validation message, no error, no console output, modal remains open — silent no-op
- Clicked after filling Project Name with "test-app"
- No response, no console output, modal remains open — silent no-op in demo mode

### Skipped (destructive)
none

### Total interactive elements found: 7
### Elements clicked: 7 (plus modal sub-element test)

## Accessibility
- Images without alt: 0
- Inputs without label: 3 (in modal dialog — `input[placeholder=my-app]`, `select` (language), `input[placeholder=.]`; labels exist as `<label>` elements but have no `for` attribute and do not wrap the inputs)
- Buttons without accessible name: 0
- Tab buttons (`.dp-view-btn`) have no `role="tab"`, no `aria-selected`, no `aria-controls` — CSS-only active state
- All 7 main content buttons have no explicit `type` attribute (defaults to `type="submit"`) and none are inside a `<form>`

## Findings

### deploy-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` has `overflow:hidden` silently clipping content. At 1920x1080: scrollWidth=1770, clientWidth=1577 (193px clipped). At 1280x800: 237px clipped. At 1024x768: 142px clipped. Content is rendered but invisible beyond the clip boundary.
- IMPACT: Users cannot see or interact with content that extends beyond the holo-panel visible area.

### deploy-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: 4 of 5 `div.dp-audit-entry` elements have `overflow:hidden; text-overflow:ellipsis; white-space:nowrap` causing text truncation. At 1920x1080: entries clip 116-315px of text. E.g. "No deployment pipelines configured. Create a pipeline to deploy your agent-built projects." (scrollWidth=554, clientWidth=239). Cloud provider descriptions are also truncated.
- IMPACT: Informational text about deployment status and cloud provider descriptions is cut off; users cannot read full content.

### deploy-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Modal dialog (`div.dp-new-dialog`) has 3 form controls with no programmatic label association. Labels exist as `<label>` elements ("Project Name", "Language", "Source Directory") but lack `for` attributes and do not wrap inputs. Selectors: `input[placeholder=my-app]`, `select`, `input[placeholder=.]`.
- IMPACT: Screen readers cannot associate labels with their form controls; users relying on assistive technology cannot identify form fields.

### deploy-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 5 tab buttons (`.dp-view-btn`: Projects, Pipeline, History, Logs, Airgap) use CSS class `active` for visual selection state but have no `role="tab"`, no `aria-selected`, and no `aria-controls`. The tab bar container has no `role="tablist"`.
- IMPACT: Screen readers announce these as generic buttons, not as tab controls; users cannot determine which tab is selected via assistive technology.

### deploy-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 7 buttons in main content have no explicit `type` attribute (defaults to `type="submit"` per HTML spec) and none are inside a `<form>`. Selectors: `.dp-new-btn` (x2), `.dp-view-btn` (x5).
- IMPACT: Buttons default to submit type outside any form context; while functionally benign, it is semantically incorrect and could cause unexpected behavior if a form wrapper is added later.

### deploy-06
- SEVERITY: P1
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Create Project" button (`.dp-form-deploy`) in the New Project modal produces no visible feedback when clicked — no validation error for empty name, no console output, no network request, modal remains open. Same silent no-op when name is filled in ("test-app"). No `hasDesktopRuntime` guard or user-facing message explains why the action fails.
- IMPACT: Users clicking "Create Project" receive zero feedback; they cannot tell if the action succeeded, failed, or requires the desktop runtime.

## Summary
- Gate detected: no
- Total interactive elements: 7
- Elements clicked: 7
- P0: 0
- P1: 2
- P2: 4
