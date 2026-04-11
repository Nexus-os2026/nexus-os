# Audit: Nexus Builder
URL: http://localhost:1420/nexus-builder
Audited at: 2026-04-09T19:55:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
- `[ModelConfigPanel] load error: Error: desktop runtime unavailable` — ModelConfigPanel.tsx:104:16 (triggered by "CHOOSE MODELS" click)
- `[NexusBuilder] model data load error: Error: desktop runtime unavailable` — NexusBuilder.tsx:260:28 (triggered by "CHOOSE MODELS" click)

### Warnings
none

### Logs
none

### Info
- `Download the React DevTools for a better development experience: https://reactjs.org/link/react-devtools` — chunk-NUMECXU6.js:21550:24

### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main: scrollWidth=1627 clientWidth=1627 [OK]
- `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=2072 clientWidth=1577 [OVERFLOW +495px horizontal, scrollHeight=1321 clientHeight=637, +684px vertical clipped]
- `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main: scrollWidth=987 clientWidth=987 [OK]
- `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=937 clientWidth=937 [OK horizontal], scrollHeight=764 clientHeight=297 [OVERFLOW +467px vertical clipped]
- `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main: scrollWidth=731 clientWidth=731 [OK]
- `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=795 clientWidth=681 [OVERFLOW +114px horizontal], scrollHeight=655 clientHeight=265 [OVERFLOW +390px vertical clipped]
- `div.living-background`: scrollWidth=1071 clientWidth=992 [OVERFLOW +79px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | ← Projects | button | — | yes |
| 2 | ✦ CHOOSE MODELS ⚙️ | button | — | yes |
| 3 | Dark portfolio / Hero, gallery, contact form | button | — | yes |
| 4 | SaaS landing page / Pricing tiers, testimonials | button | — | yes |
| 5 | Restaurant site / Menu, reservations | button | — | yes |
| 6 | Personal blog / Dark mode, code blocks | button | — | yes |
| 7 | (textarea) "Describe the website you want to build..." | textarea | — | yes |
| 8 | ⚡ Build It | button | — | no (disabled until textarea has content) |
| 9 | mobile | button | — | yes |
| 10 | tablet | button | — | yes |
| 11 | desktop | button | — | yes |
| 12 | preview | button | — | yes |
| 13 | code | button | — | yes |
| 14 | ↻ (Refresh) | button | — | no (disabled) |
| 15 | ↓ (Download HTML) | button | — | no (disabled) |

## Click sequence
### Click 1: "← Projects"
- Pathname before: /nexus-builder
- New console: clean
- Network failures: none
- Visible change: none — button is a no-op; page content unchanged
- Pathname after: /nexus-builder
- Reverted: n/a

### Click 2: "✦ CHOOSE MODELS ⚙️"
- Pathname before: /nexus-builder
- New console: ERROR `[ModelConfigPanel] load error: Error: desktop runtime unavailable` (ModelConfigPanel.tsx:104:16), ERROR `[NexusBuilder] model data load error: Error: desktop runtime unavailable` (NexusBuilder.tsx:260:28)
- Network failures: none (Tauri invoke, not HTTP)
- Visible change: Model Configuration panel appears with error state: "⚠️ Failed to load model configuration. desktop runtime unavailable" with Retry button and X close
- Pathname after: /nexus-builder
- Reverted: n/a (closed panel via X button)

### Click 3: "Dark portfolio"
- Pathname before: /nexus-builder
- New console: clean
- Network failures: none
- Visible change: textarea populated with "Build a dark portfolio website with animated hero, project gallery with hover effects, skills section, and contact form..."
- Pathname after: /nexus-builder
- Reverted: n/a

### Click 4: "SaaS landing page"
- Pathname before: /nexus-builder
- New console: clean
- Network failures: none
- Visible change: none — textarea still shows Dark portfolio text (template did not replace)
- Pathname after: /nexus-builder
- Reverted: n/a

### Click 5: "Restaurant site"
- Pathname before: /nexus-builder
- New console: clean
- Network failures: none
- Visible change: none — textarea still shows Dark portfolio text
- Pathname after: /nexus-builder
- Reverted: n/a

### Click 6: "Personal blog"
- Pathname before: /nexus-builder
- New console: clean
- Network failures: none
- Visible change: none — textarea still shows Dark portfolio text (but React state DID update — Build It later used "Personal blog" prompt)
- Pathname after: /nexus-builder
- Reverted: n/a

### Click 7: "⚡ Build It"
- Pathname before: /nexus-builder
- New console: clean
- Network failures: none (Tauri invoke failure, not HTTP)
- Visible change: Build pipeline initiated — shows "Planning your build / Haiku 4.5 analyzing prompt" then "✗ Plan failed: desktop runtime unavailable". Used "Personal blog" prompt despite textarea showing "Dark portfolio" text, confirming state/DOM mismatch. Project history sidebar appeared with "· Create a personal blog with article" entry.
- Pathname after: /nexus-builder
- Reverted: n/a

### Click 8: "mobile"
- Pathname before: /nexus-builder
- New console: clean
- Network failures: none
- Visible change: none visible — no preview iframe exists to resize
- Pathname after: /nexus-builder
- Reverted: n/a

### Click 9: "tablet"
- Pathname before: /nexus-builder
- New console: clean
- Network failures: none
- Visible change: none visible
- Pathname after: /nexus-builder
- Reverted: n/a

### Click 10: "desktop"
- Pathname before: /nexus-builder
- New console: clean
- Network failures: none
- Visible change: none visible
- Pathname after: /nexus-builder
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 15
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (textarea has placeholder but no `aria-label`, no `id`, no associated `<label>`)
- Buttons without accessible name: 0 (all buttons have text content; however 2 buttons use unicode-only text ↻ and ↓ with `title` attribute but no `aria-label` — screen readers may announce the unicode character rather than the intended label)

## Findings

### nexus-builder-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel` has `overflow: hidden` with fixed computed height. At 1920x1080: scrollWidth=2072 vs clientWidth=1577 (+495px horizontal), scrollHeight=1321 vs clientHeight=637 (+684px vertical clipped). At 1280x800: +467px vertical clipped. At 1024x768: +114px horizontal, +390px vertical clipped. Content below the fold is invisible and unreachable.
- IMPACT: Builder UI content is silently clipped at all viewports — users cannot scroll to reach lower portions of the interface.

### nexus-builder-02
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: Quick-start template buttons (SaaS, Restaurant, Personal blog) update React state but do not update the textarea's DOM value when a template is already loaded. After clicking "Dark portfolio" (populates textarea), clicking "Personal blog" leaves textarea showing Dark portfolio text, but "Build It" uses the Personal blog prompt.
- IMPACT: State/DOM mismatch — user sees one prompt in textarea but a different prompt is submitted to the build pipeline, causing confusion.

### nexus-builder-03
- SEVERITY: P2
- DIMENSION: console
- VIEWPORT: 1920
- EVIDENCE: Clicking "CHOOSE MODELS" triggers two console errors: `[ModelConfigPanel] load error: Error: desktop runtime unavailable` at ModelConfigPanel.tsx:104:16 and `[NexusBuilder] model data load error: Error: desktop runtime unavailable` at NexusBuilder.tsx:260:28. The panel renders an error state with "⚠️ Failed to load model configuration."
- IMPACT: Expected in demo mode — model configuration panel gracefully shows error state with retry option. No crash.

### nexus-builder-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows at all viewports: +151px at 1920x1080, +100px at 1280x800, +79px at 1024x768. The background canvas/div extends beyond the viewport width.
- IMPACT: Cosmetic — background element extends beyond viewport but does not cause visible scrollbar (parent clips it).

### nexus-builder-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: "← Projects" button (btns[0] in main) fires with no console output, no navigation, and no visible change. `window.location.pathname` remains `/nexus-builder`.
- IMPACT: Back navigation button is a no-op — user cannot return to a projects list view.

### nexus-builder-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: A `<span>` element with text "Loading..." (styled `font-size: 11px; color: rgb(62, 76, 94)`) is permanently visible in the main content area. It persists across all interactions and never resolves.
- IMPACT: Perpetual loading indicator suggests content that will never arrive in demo mode, misleading users about page state.

### nexus-builder-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two icon-only buttons (↻ Refresh, ↓ Download HTML) have `title` attributes but no `aria-label`. Their text content is a single unicode character. Screen readers may announce "↻" or "↓" rather than "Refresh" or "Download HTML". The textarea has `placeholder` but no `aria-label`, `id`, or associated `<label>` element.
- IMPACT: Screen reader users cannot determine the purpose of icon buttons; textarea is not programmatically labeled per WCAG 1.3.1/4.1.2.

### nexus-builder-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: NexusBuilder injects an inline `<style>` tag with `@keyframes nbspin`, `@keyframes nbpulse`, `@keyframes nbfadein` directly into the main content DOM rather than using a CSS file or CSS-in-JS solution.
- IMPACT: Style pollution — inline keyframes injected on every mount; could conflict with identically named keyframes elsewhere.

## Summary
- Gate detected: no
- Total interactive elements: 15
- Elements clicked: 10
- P0: 0
- P1: 1
- P2: 7
