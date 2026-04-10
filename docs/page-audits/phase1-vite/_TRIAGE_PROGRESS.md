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
