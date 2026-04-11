# Phase 1 Vite Audit — Triage Progress

## Cluster A: `div.living-background` width overflow

**Status:** DONE
**Fixed:** 2026-04-10
**Commit:** (see git log)
**Pages affected:** 88

### Root cause

`.living-background__grid` used `inset: -8%` which extended the grid element 148px beyond the parent's right edge. The parent's `overflow: hidden` clipped it visually, but `scrollWidth` still reported the full content width (2004 vs clientWidth 1856).

### Fix

Removed `inset: -8%` from `.living-background__grid` in `app/src/styles/fx.css`. The grid now inherits `inset: 0` from the shared child rule, keeping it exactly viewport-sized. The `-8%` extension was a safety margin for the parallax effect, but the mask-image radial gradient already fades to transparent at 88%, making the extra coverage unnecessary.

### Before / After

| Viewport | Before scrollWidth | Before clientWidth | After scrollWidth | After clientWidth | Match |
|----------|-------------------|-------------------|------------------|------------------|-------|
| Dashboard | 2004 | 1856 | 1856 | 1856 | true |
| /files | — | — | 1856 | 1856 | true |
| /api-client | — | — | 1856 | 1856 | true |

One-line diff: removal of `inset: -8%` from `.living-background__grid`.

## Cluster B: `section.holo-panel.holo-panel-mid` clips its children

**Status:** DONE
**Fixed:** 2026-04-10
**Commit:** (see git log)
**Pages affected:** 88

### Root cause

`.holo-panel__refraction` used `inset: -50% -20%` (extending ~315px past each side) with a `holo-drift` rotation animation. Chrome includes both the negative-inset layout overflow AND the rotation-transformed bounding box in the parent's `scrollWidth`, even with `overflow: hidden` or `overflow: clip`. The rotating element's bounding box fluctuated frame-to-frame, causing scrollWidth to vary between ~1640–2100 depending on rotation angle.

### Fix

Three CSS changes in `app/src/styles/fx.css`:

1. `.holo-panel`: changed `overflow: hidden` to `overflow: clip` — same visual clipping but the element is no longer a scroll container
2. `.holo-panel__refraction`: restructured as a clipping container with `inset: 0; overflow: clip` — holds the `mix-blend-mode: screen` and `pointer-events: none`
3. `.holo-panel__refraction::before`: new pseudo-element that carries the conic-gradient background and rotation animation at the original `inset: -50% -20%` size, safely contained within the refraction wrapper

The refraction's visual effect is identical — the oversized rotating gradient is now clipped by its own parent (`overflow: clip`) rather than by the holo-panel directly, preventing the rotation transform from inflating the panel's `scrollWidth`.

### Before / After

| Page | Before scrollWidth | Before clientWidth | After scrollWidth | After clientWidth | Match |
|------|-------------------|-------------------|------------------|------------------|-------|
| /dashboard | 2083 | 1577 | 1577 | 1577 | true |
| /files | ~1716 | 1577 | 1577 | 1577 | true |
| /api-client | 2087 | 1577 | 1577 | 1577 | true |

Stable across animation frames (verified 6 consecutive frames, all match).

## Cluster H: Leaked `[TEST]` debug code in component files

**Status:** DONE
**Fixed:** 2026-04-10
**Commit:** (see git log)
**Files affected:** 1 (Agents.tsx — all 4 audit findings traced back to this single file)

### Files and classifications

| File | Classification | Action |
|------|----------------|--------|
| src/pages/Agents.tsx (useEffect, lines 219-228) | Type 1 — Leaked test code | Deleted entire `TEMPORARY DIAGNOSTIC` useEffect block |
| src/pages/Agents.tsx (button, lines 551-576) | Type 1 — Leaked test code | Deleted entire `TEST EMIT` button element |

### Root cause

Two blocks of debug code labeled `TEMPORARY DIAGNOSTIC` were left in `Agents.tsx` from development. The first was a standalone `listen("agent-cognitive-cycle")` useEffect that only logged events to console. The second was a hot-pink `TEST EMIT` button that invoked a test Tauri command (`test_emit_event`). Both fired `[TEST]` console.log messages on every page load, and the listener threw `transformCallback` TypeError in Vite/demo mode because `window.__TAURI_IPC__` doesn't exist outside Tauri runtime.

### Fix

Deleted both `TEMPORARY DIAGNOSTIC` blocks entirely (Type 1 classification — pure debug code with no production purpose). Also removed the now-unused `import { invoke } from "@tauri-apps/api/core"` that was only consumed by the test button. The `listen` import remains as it's used by 4 real production listeners in the same file.

### Console verification

- Before: `[TEST] attaching standalone test listener` + `[TEST] standalone listen failed: TypeError: Cannot read properties of undefined (reading 'transformCallback')` firing twice per page load (React StrictMode)
- After: clean console on /agents — no `[TEST]` messages, no `transformCallback` errors, no new errors introduced

## Cluster F: Icon buttons use `title` instead of `aria-label`

**Status:** DONE
**Fixed:** 2026-04-10
**Commit:** (see git log)
**Buttons affected:** 8 (across 6 files)

### Buttons and classifications

| File | Line | Button purpose | Classification | Action |
|------|------|----------------|----------------|--------|
| src/pages/FileManager.tsx | 336 | Refresh (F5) | Type 1 | Added aria-label, kept title |
| src/pages/FileManager.tsx | 353 | Go up | Type 1 | Added aria-label, kept title |
| src/pages/Terminal.tsx | 568 | New Tab (Ctrl+T) | Type 1 | Added aria-label, kept title |
| src/pages/ApiClient.tsx | 303 | New collection | Type 1 | Added aria-label, kept title |
| src/pages/AiChatHub.tsx | 1377 | Copy conversation | Type 1 | Added aria-label, kept title |
| src/pages/AiChatHub.tsx | 1676 | Generate image | Type 1 | Added aria-label, kept title |
| src/pages/NexusBuilder.tsx | 1432 | Download HTML (↓) | Type 1 | Added aria-label, kept title |
| src/components/builder/PropertyPanel.tsx | 267 | Deselect (×) | Type 2 | Replaced title with aria-label |

### Root cause

Developers used the HTML `title` attribute as the only accessible name for icon-only buttons. Screen readers handle `title` inconsistently — some announce it after a delay, some only on focus, some not at all. The correct attribute for accessible names is `aria-label`.

### Fix

7 buttons classified as Type 1 (toolbar/action icons where hover tooltips are useful): kept existing `title` attribute and added matching `aria-label`. 1 button classified as Type 2 (× close/deselect button where meaning is obvious): replaced `title` with `aria-label` since a tooltip is redundant for a standard close affordance.

### Verification

- Hover tooltips still appear on Type 1 buttons (verified on /api-client "New collection")
- Accessibility tree now shows accessible names for all fixed buttons
- Build passes

## Cluster C: `span.nexus-sidebar-item-text` 4px text clipping

**Status:** DONE
**Fixed:** 2026-04-10
**Commit:** (see git log)
**Pages affected:** 88 (sidebar appears on every page)

### Root cause

`.nexus-sidebar-shortcut` (keyboard shortcut badges like "Alt+1") had `transform: translateX(4px)` as default state, sliding to `translateX(0)` on hover. Even though the shortcut was `opacity: 0` (invisible), Chrome included the 4px transform offset in the parent `.nexus-sidebar-item-text`'s scrollWidth. Only the 3 sidebar items with shortcuts (Dashboard, Chat, Agents) actually overflowed — the other 85 items had no shortcut text and no overflow.

### Fix

Removed `transform: translateX(4px)` from `.nexus-sidebar-shortcut` default state and `transform: translateX(0)` from the hover state in `app/src/components/layout/sidebar.css`. The shortcut now fades in with opacity only (160ms ease transition), without the subtle slide-in effect. The slide was barely perceptible alongside the opacity transition.

### Before / After

| Page | Before overflowing | After overflowing |
|------|-------------------|------------------|
| /dashboard | 3 of 88 spans | 0 of 88 spans |
| /files | 3 of 88 spans | 0 of 88 spans |
| /api-client | 3 of 88 spans | 0 of 88 spans |

## Cluster D: `<button type="submit">` outside `<form>` elements

**Status:** DONE
**Fixed:** 2026-04-10
**Commit:** (see git log)
**Files affected:** 117 (across 88 pages + shared components)

### Root cause

Buttons declared without an explicit `type` attribute default to `type="submit"` in HTML. Across the codebase, ~500 buttons outside `<form>` elements had no `type` attribute, making them implicitly `type="submit"` — semantically incorrect and a latent form-submission risk if a parent form is ever added.

### Experiment result

Ran a controlled experiment on Dashboard before rolling out. Changed Refresh and Start Jarvis buttons from implicit `type="submit"` to `type="button"`. Result: **INDEPENDENT** from Cluster E. Dead-button behavior was unchanged — buttons are dead because their handlers call Tauri IPC commands unavailable in demo mode, not because of the button type. D and E require separate fixes.

### Fix

Mechanical sweep: added `type="button"` to all `<button>` elements outside `<form>` elements across 117 files. Three buttons inside forms (Chat.tsx, BackendPanel.tsx x2) were correctly left as `type="submit"`. No behavioral changes — purely semantic correctness.

### Verification

| Page | Total buttons | Missing type attr |
|------|--------------|-------------------|
| /files | 111 | 0 |
| /api-client | 103 | 0 |
| /agents | 101 | 0 |

Build passes. No new console errors on any spot-checked page.

## Cluster E: Shell header demo-mode feedback (Refresh + Start Jarvis)

**Status:** ALREADY FIXED (no code change needed)
**Verified:** 2026-04-10
**Files affected:** 0 (no changes required)

### Root cause (of the audit finding, not a bug)

The audit reported "dead buttons — zero feedback on click" for Refresh and Start Jarvis. Investigation found that demo-mode feedback **already exists**: `showDemoToast()` in App.tsx (line 890) sets a `runtimeError` state rendering an error badge: "Action unavailable in demo mode — requires desktop backend". Both `handleRefresh()` (line 1425) and `enableJarvisMode()` (line 1443) call `showDemoToast()` when `runtimeMode !== "desktop"`.

The automated Puppeteer audit script's change-detection heuristic did not capture the error badge DOM insertion, so it reported "no visible change." Manual browser verification confirms the feedback works correctly on all tested pages.

### Verification

| Page | Refresh click | Start Jarvis click |
|------|--------------|-------------------|
| /dashboard | Error badge shown | Error badge shown |
| /files | Error badge shown | — |
| /agents | Error badge shown | — |

No code changes required. Cluster E is resolved by the existing implementation.

---

## Phase 2A Complete

All 7 clusters triaged and resolved:

| Cluster | Finding | Fix type | Commit |
|---------|---------|----------|--------|
| A | living-background overflow | CSS: removed grid inset | 898ad4e9 |
| B | holo-panel clips children | CSS: refraction → pseudo-element | 65880913 |
| C | sidebar text clipping | CSS: removed shortcut translateX | abe7f84c |
| D | type="submit" outside form | JSX: added type="button" | 0961ed4c |
| E | dead buttons (no feedback) | Already fixed (showDemoToast) | n/a |
| F | icon buttons use title not aria-label | JSX: added aria-label | c2733a7b |
| H | leaked [TEST] debug code | JSX: deleted test blocks | 9903dd91 |

Ready for single CI push to remote.

## Phase 2B — False Positives

### agents-start-jarvis-banner
**Status:** FALSE POSITIVE (no code change needed)
**Verified:** 2026-04-11
**Same class as:** Cluster E (dashboard-03)

Audit reported "clicking Start Jarvis silently dismisses Desktop runtime banner with no feedback." Direct testing on HEAD confirmed:
1. RequiresLlm gate persists after click (not dismissed)
2. Shell-level showDemoToast() fires the "Action unavailable in demo mode — requires desktop backend" error badge correctly
3. No page-level Start Jarvis handler exists in Agents.tsx — the only button is in the shell header, already covered by Cluster E's existing implementation

Root cause of the audit finding: the Puppeteer change-detection heuristic does not capture the transient error badge DOM insertion, identical to the issue behind Cluster E. This reinforces that the audit script change-detection (Phase 2B queue item 6) must be fixed before further audit-finding triage to stop burning investigation time on non-bugs.
