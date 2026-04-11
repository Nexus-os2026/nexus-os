# Audit: Agent Memory
URL: http://localhost:1420/agent-memory
Audited at: 2026-04-09T22:57:00Z
Gate detected: false
Gate type: none

## Console (captured at 1248x671, viewport resize unavailable)
### Errors
none

### Warnings
- `[AgentMemory] Error: desktop runtime unavailable` at AgentMemory.tsx:95:39 — listAgents call via backend.ts:17 invokeDesktop (x2, React strict-mode double-invoke)
- `[AgentMemory] Error: desktop runtime unavailable` at AgentMemory.tsx:98:39 — memoryGetPolicy call via backend.ts:2725 (x2, React strict-mode double-invoke)

### Logs
none

### Info
- `Download the React DevTools for a better development experience` at chunk-NUMECXU6.js:21550:24

### Debug
- `[vite] connecting...` at @vite/client:494:8
- `[vite] connected.` at @vite/client:617:14

## Overflow

Note: resize_window tool has no effect in this environment. All three viewport measurements are at the actual viewport 1248x671.

### 1920x1080 (measured at 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]

### 1280x800 (measured at 1248x671)
- (same as above — resize had no effect)

### 1024x768 (measured at 1248x671)
- (same as above — resize had no effect)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Select Agent | select (combobox) | — | yes |
| 2 | Save | button (type=submit) | — | yes |
| 3 | Load | button (type=submit) | — | yes |
| 4 | Consolidate | button (type=submit) | — | yes |
| 5 | Refresh | button (type=submit) | — | yes |
| 6 | Semantic (memory type) | select (combobox) | — | yes |
| 7 | Memory summary... | textarea | — | yes |
| 8 | Tags (comma-separated) | input[text] | — | yes |
| 9 | Domain | input[text] | — | yes |
| 10 | Importance (0.7) | input[range] | — | yes |
| 11 | Store Memory | button (type=submit) | — | no (disabled) |
| 12 | Search query... | input[text] | — | yes |
| 13 | All Types (search filter) | select (combobox) | — | yes |
| 14 | Search | button (type=submit) | — | yes |
| 15 | Task description... | input[text] | — | yes |
| 16 | Preview | button (type=submit) | — | no (disabled) |

## Click sequence
### Click 1: "Select Agent"
- Pathname before: /agent-memory
- New console: clean
- Network failures: none
- Visible change: Select opened; only 1 option ("Select Agent" placeholder) — no agent options available
- Pathname after: /agent-memory
- Reverted: n/a

### Click 2: "Save"
- Pathname before: /agent-memory
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /agent-memory
- Reverted: n/a

### Click 3: "Load"
- Pathname before: /agent-memory
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /agent-memory
- Reverted: n/a

### Click 4: "Consolidate"
- Pathname before: /agent-memory
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /agent-memory
- Reverted: n/a

### Click 5: "Refresh"
- Pathname before: /agent-memory
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /agent-memory
- Reverted: n/a

### Click 6: "Semantic" (memory type select)
- Pathname before: /agent-memory
- New console: clean
- Network failures: none
- Visible change: Select opened; 4 options available (Episodic, Semantic, Procedural, Relational)
- Pathname after: /agent-memory
- Reverted: n/a

### Click 7: "Store Memory" (disabled)
- Pathname before: /agent-memory
- New console: clean
- Network failures: none
- Visible change: none — button disabled, click ignored
- Pathname after: /agent-memory
- Reverted: n/a

### Click 8: "Search"
- Pathname before: /agent-memory
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /agent-memory
- Reverted: n/a

### Click 9: "All Types" (search filter select)
- Pathname before: /agent-memory
- New console: clean
- Network failures: none
- Visible change: Select opened; 5 options (All Types, Episodic, Semantic, Procedural, Relational)
- Pathname after: /agent-memory
- Reverted: n/a

### Click 10: "Preview" (disabled)
- Pathname before: /agent-memory
- New console: clean
- Network failures: none
- Visible change: none — button disabled, click ignored
- Pathname after: /agent-memory
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 16
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 9 (selectors: `select` (agent), `select` (memory type), `textarea[placeholder="Memory summary..."]`, `input[type=text][placeholder="Tags (comma-separated)"]`, `input[type=text][placeholder="Domain"]`, `input[type=range]` (importance), `input[type=text][placeholder="Search query..."]`, `select` (search type filter), `input[type=text][placeholder="Task description..."]`)
- Buttons without accessible name: 0
- Additional: Duplicate H1 elements ("Memory Store" in banner header, "Persistent Agent Memory" in main content)
- Additional: 1 unlabeled `<section>` in main content (no aria-label or aria-labelledby)

## Findings

### agent-memory-01
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 9 form controls (`select`, `textarea`, `input[text]` x4, `input[range]`, `select` x2) have no `id`, no `name`, no `<label>` association, no `aria-label`, and no `aria-labelledby`. They rely solely on proximity to visible text or placeholder attributes for identification.
- IMPACT: Screen readers cannot programmatically associate labels with controls; form data cannot be serialized by name.

### agent-memory-02
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page: "Memory Store" (in `banner` region header) and "Persistent Agent Memory" (in `main` content). Document should have a single H1 per page.
- IMPACT: Screen readers and SEO crawlers receive ambiguous page heading hierarchy.

### agent-memory-03
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has `overflow: hidden` with scrollHeight=749 but clientHeight=353, clipping 396px of vertical content. The "Search Memories", "Context Preview", and "Memories (0)" sections are hidden below the fold with no scroll mechanism.
- IMPACT: Users cannot see or interact with content below the visible portion of the holo-panel; the Search, Context Preview, and Memories list sections may be partially or fully clipped.

### agent-memory-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` has scrollWidth=1348 vs clientWidth=1248, overflowing by 100px horizontally. This is `position: fixed` with `overflow: hidden`, so not user-visible but indicates sizing issue.
- IMPACT: Cosmetic — no visible scrollbar due to fixed positioning and overflow:hidden, but indicates the background element is wider than the viewport.

### agent-memory-05
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel` has scrollWidth=1019 vs clientWidth=937, overflowing by 82px horizontally. With `overflow: hidden`, this content is clipped without any scroll affordance.
- IMPACT: Some horizontal content within the holo-panel may be silently clipped at narrower viewports.

### agent-memory-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 7 buttons in main content use `type="submit"` (Save, Load, Consolidate, Refresh, Store Memory, Search, Preview). These are not inside `<form>` elements with submit handlers; they should use `type="button"`.
- IMPACT: Pressing Enter in a text input may trigger unintended form submission behavior; semantically incorrect button type.

### agent-memory-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Select Agent" combobox renders with only 1 option (the placeholder "Select Agent"). No agent options are populated in demo mode. Buttons Save, Load, Consolidate, Refresh, Search all produce zero feedback when clicked — no console output, no visible state change, no error message.
- IMPACT: Users see a fully-rendered form surface but cannot perform any action; no feedback indicates why operations fail silently.

### agent-memory-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 1 `<section>` element in main content has no `aria-label` or `aria-labelledby` attribute.
- IMPACT: Screen readers announce a generic unnamed region, reducing navigability.

## Summary
- Gate detected: no
- Total interactive elements: 16
- Elements clicked: 10
- P0: 0
- P1: 3
- P2: 5
