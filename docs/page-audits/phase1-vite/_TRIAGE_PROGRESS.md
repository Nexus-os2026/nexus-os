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
