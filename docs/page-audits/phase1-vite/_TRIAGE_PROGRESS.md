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
