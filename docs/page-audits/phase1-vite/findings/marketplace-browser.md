# Audit: Marketplace Browser
URL: http://localhost:1420/marketplace-browser
Audited at: 2026-04-10T02:01:00Z
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
(actual CSS viewport: 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px, overflow-x: hidden]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]

### 1280x800
CSS viewport resize not achievable via MCP `resize_window` / `setDeviceMetricsOverride` in this environment (confirmed in prior audits: obs 1319, 1320). Measurements deferred to Puppeteer pass.

### 1024x768
Same limitation as above. Measurements deferred to Puppeteer pass.

## Interactive elements (main content only)

Initial load (Pre-installed tab active by default):

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Search by name, description, or capability... | input[type=text] | — | yes |
| 2 | All | button[type=button] | — | yes |
| 3 | L1 | button[type=button] | — | yes |
| 4 | L2 | button[type=button] | — | yes |
| 5 | L3 | button[type=button] | — | yes |
| 6 | L4 | button[type=button] | — | yes |
| 7 | L5 | button[type=button] | — | yes |
| 8 | L6 | button[type=button] | — | yes |
| 9 | Pre-installed | button (no type) | — | yes |
| 10 | Community (GitLab) | button (no type) | — | yes |

After clicking "Community (GitLab)", 2 additional elements appear:

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 11 | Search nexus-agent repos on GitLab... | input[type=text] | — | yes |
| 12 | Search GitLab | button (no type) | — | yes |

## Click sequence
### Click 1: "Search input"
- Pathname before: /marketplace-browser
- New console: clean
- Network failures: none
- Visible change: Input receives focus
- Pathname after: /marketplace-browser
- Reverted: n/a

### Click 2: "All" (filter button)
- Pathname before: /marketplace-browser
- New console: clean
- Network failures: none
- Visible change: "All" button gains `.active` class (CSS-only state toggle)
- Pathname after: /marketplace-browser
- Reverted: n/a

### Click 3: "L1" (filter button)
- Pathname before: /marketplace-browser
- New console: clean
- Network failures: none
- Visible change: "L1" gains active state, filters agent list
- Pathname after: /marketplace-browser
- Reverted: n/a

### Click 4: "L2" (filter button)
- Pathname before: /marketplace-browser
- New console: clean
- Network failures: none
- Visible change: "L2" gains active state, filters agent list
- Pathname after: /marketplace-browser
- Reverted: n/a

### Click 5: "L3" (filter button)
- Pathname before: /marketplace-browser
- New console: clean
- Network failures: none
- Visible change: "L3" gains active state, filters agent list
- Pathname after: /marketplace-browser
- Reverted: n/a

### Click 6: "L4" (filter button)
- Pathname before: /marketplace-browser
- New console: clean
- Network failures: none
- Visible change: "L4" gains active state, filters agent list
- Pathname after: /marketplace-browser
- Reverted: n/a

### Click 7: "L5" (filter button)
- Pathname before: /marketplace-browser
- New console: clean
- Network failures: none
- Visible change: "L5" gains active state, filters agent list
- Pathname after: /marketplace-browser
- Reverted: n/a

### Click 8: "L6" (filter button)
- Pathname before: /marketplace-browser
- New console: clean
- Network failures: none
- Visible change: "L6" gains active state, filters agent list
- Pathname after: /marketplace-browser
- Reverted: n/a

### Click 9: "Pre-installed" (section tab)
- Pathname before: /marketplace-browser
- New console: clean
- Network failures: none
- Visible change: No visible active state on button; content does not visibly change
- Pathname after: /marketplace-browser
- Reverted: n/a

### Click 10: "Community (GitLab)" (section tab)
- Pathname before: /marketplace-browser
- New console: clean
- Network failures: none
- Visible change: Community tab content revealed — new search input ("Search nexus-agent repos on GitLab...") and "Search GitLab" button appear. Tab button itself does NOT gain `.active` class.
- Pathname after: /marketplace-browser
- Reverted: n/a

### Bonus Click 11: "Search GitLab" (button, appeared after click 10)
- Pathname before: /marketplace-browser
- New console: clean
- Network failures: none
- Visible change: No visible change — silent no-op in demo mode
- Pathname after: /marketplace-browser
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 10 (initial load), 12 (after Community tab)
### Elements clicked: 11 (10 initial + 1 dynamically revealed)

## Accessibility
- Images without alt: 0
- Inputs without label: 2 (selectors: `input.as-search[placeholder="Search by name, description, or capability..."]`, `input[placeholder="Search nexus-agent repos on GitLab..."]`)
- Buttons without accessible name: 0
- Sections without aria-label/role: 4 (selectors: `section.holo-panel`, `section.as-page`, `section.as-section` x2)
- Buttons missing `type` attribute: 3 ("Pre-installed", "Community (GitLab)", "Search GitLab") — default to `type="submit"`
- Filter buttons (All, L1-L6) lack `aria-pressed` or `aria-selected` attributes — active state is CSS-only
- `h1` ("Browse Agents") is outside `<main>` landmark

## Findings

### marketplace-browser-01
- SEVERITY: P1
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: Route `/marketplace-browser` renders `section.as-page` — the same Agent Store component used at `/agent-store` and `/publish`. The h1 reads "Browse Agents", the h2 reads "PRE-INSTALLED AGENTS", and the section title reads "Unified runtime + community marketplace". The page has class `as-page` (Agent Store prefix). All interactive elements match the Agent Store UI identically.
- IMPACT: Three separate routes (`/agent-store`, `/publish`, `/marketplace-browser`) render the same Agent Store component — users navigating to "Marketplace Browser" see agent-store content with no marketplace-browser-specific UI.

### marketplace-browser-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `div.living-background` scrollWidth=2039 > clientWidth=1888 (+151px). Fixed-position background element is wider than viewport.
- IMPACT: No visible scrollbar (element is a visual background), but creates a potential layout shift target if `overflow: hidden` is ever removed from ancestor.

### marketplace-browser-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `section.holo-panel` scrollWidth=1716 > clientWidth=1577 (+139px). overflow-x is set to `hidden`, masking the overflow.
- IMPACT: Content inside holo-panel is clipped by 139px — potential data loss if panel contains right-aligned interactive elements.

### marketplace-browser-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<input type="text">` elements lack `<label>`, `aria-label`, or `aria-labelledby`. Selectors: `input.as-search` (main search) and the GitLab search input (no class, placeholder "Search nexus-agent repos on GitLab...").
- IMPACT: Screen readers announce these inputs without an accessible name; fails WCAG 2.1 SC 1.3.1 and 4.1.2.

### marketplace-browser-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 4 `<section>` elements lack `aria-label`, `aria-labelledby`, or `role` attributes: `section.holo-panel`, `section.as-page`, `section.as-section` (x2).
- IMPACT: Screen readers cannot distinguish section purposes; fails WCAG 2.1 SC 1.3.1 landmark requirements.

### marketplace-browser-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<h1>Browse Agents</h1>` is rendered outside `<main class="nexus-shell-content">`. The h1 sits in the shell header, not inside the main landmark.
- IMPACT: Screen reader users navigating by landmark will not find the page heading inside the main content region.

### marketplace-browser-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 3 buttons missing `type` attribute: "Pre-installed", "Community (GitLab)", "Search GitLab". Per HTML spec, `<button>` without `type` defaults to `type="submit"`, which can cause unintended form submission if wrapped in a `<form>`.
- IMPACT: Semantic mismatch — tab/action buttons should be `type="button"`.

### marketplace-browser-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Filter buttons (All, L1-L6) toggle CSS class `.active` on click but have no `aria-pressed`, `aria-selected`, or `role="tab"` attributes. Active state is purely visual.
- IMPACT: Screen reader users cannot determine which filter is currently active; fails WCAG 2.1 SC 4.1.2.

### marketplace-browser-09
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Section tab buttons "Pre-installed" and "Community (GitLab)" do not receive an `.active` CSS class when clicked, unlike the filter buttons (All, L1-L6) which do. After clicking "Community (GitLab)", neither tab shows active state.
- IMPACT: Users cannot visually determine which section tab is currently selected.

### marketplace-browser-10
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Search GitLab" button click produces no console output, no network request, and no visible change. Silent no-op in demo mode.
- IMPACT: Button appears interactive but provides no feedback — user cannot tell if the action was attempted or ignored.

## Summary
- Gate detected: no
- Total interactive elements: 12 (10 initial + 2 revealed by Community tab)
- Elements clicked: 11
- P0: 0
- P1: 1
- P2: 9
