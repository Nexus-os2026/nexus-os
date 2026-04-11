# Phase 2B Queue

Carried over from Phase 2A (audit + cluster fix campaign). Items here are real findings that were intentionally deferred — either because they need per-page investigation, belong to a different fix class, or require the full Tauri runtime to reproduce.

**Status:** Open
**Created:** 2026-04-10 (end of Phase 2A)
**Predecessor:** `_TRIAGE_PROGRESS.md` (Phase 2A, 7 clusters complete)

---

## P1 — Real bugs

### 1. `dashboard-01` residual overflow (+28px)
**Source:** Cluster A post-fix re-audit on Dashboard.
**Before Cluster A:** scrollWidth=2039, clientWidth=1888 (+151px)
**After Cluster A:** scrollWidth=1405, clientWidth=1377 (+28px)
**Status:** Cluster A's `living-background` grid inset removal closed most of the overflow but a separate 28px source remains. Different root cause. Needs DOM inspection on Dashboard specifically — likely a sibling element or a different transform.
**Action:** Open Dashboard in DevTools, walk the layout tree, find the element contributing the residual 28px, file as its own fix.

### 2. `api-client` sidebar stuck in Audit view
**Source:** Phase 1 audit, flagged as hardest-hit page.
**Symptom:** Sidebar state-management bug — once user navigates into Audit view, sidebar doesn't reset on subsequent navigation.
**Action:** Real state-management bug, needs investigation in `app/src/pages/ApiClient.tsx` and whatever store backs the sidebar state.

### 3. `agents` — dead settings link
**File:** `app/src/pages/Agents.tsx`
**Symptom:** "I have an API key" CTA renders as `<a href="#/settings">`. Page uses path-based routing, not hash routing. Click does nothing.
**Fix:** Replace with router `<Link to="/settings">` or equivalent navigation call.

### 4. `agents` — Start Jarvis dismisses banner with no feedback
**File:** `app/src/pages/Agents.tsx`
**Symptom:** Clicking Start Jarvis silently dismisses the "Desktop runtime required" warning banner. No toast, no replacement state. Confusing UX.
**Note:** Cluster E's `showDemoToast()` should fire here — verify whether it does and the banner-dismiss is the actual bug, or whether the handler path on Agents bypasses `showDemoToast()`.

---

## P2 — Hardest-hit pages flagged for full per-page sweep

These pages had the highest P1 finding density in the Phase 1 audit. Group remaining ~290 P2 findings by feature area and triage page-by-page:

| Page | P1 count | Notes |
|------|----------|-------|
| `governed-control` | 4 | |
| `world-sim` | 4 | |
| `consciousness` | 4 | |
| `media` | 5 | |
| `api-client` | — | Already flagged above for sidebar bug |

---

## Trivial

### 5. `favicon.ico` 404 on every page
**Symptom:** Browser requests `/favicon.ico`, server returns 404. Logged on every audit page.
**Fix:** Either add a favicon to `app/public/favicon.ico` or remove the `<link rel="icon">` reference from `index.html`.

---

## Audit infrastructure improvements

**Status:** Done — 2026-04-11 (commit lands with this change)

### 6. Audit script change-detection misses DOM-inserted badges
**Source:** Cluster E false positive.
**Symptom:** Puppeteer audit driver flagged Refresh + Start Jarvis as "dead buttons" on 67 pages because its change-detection heuristic compares screenshots/DOM states and missed the `nx-badge-error` div that `showDemoToast()` inserts into the header. The error badge auto-clears after 3s, which may also have contributed.
**Action:** Before Phase 2C, improve `scripts/page-audit/` change detection:
- Watch for new DOM nodes anywhere on the page within N ms of click, not just in a fixed region
- Watch for transient elements (mutation observer) instead of pre/post screenshot diff
- Account for elements that auto-clear within the polling window
**File:** `scripts/page-audit/run_batch.sh` and the per-page audit driver it calls.

---

## Architectural pattern to investigate

### 7. CSS transform offsets included in `scrollWidth` for visually-hidden elements
**Source:** Root cause class shared by Cluster B (`holo-panel` rotating conic-gradient) and Cluster C (sidebar shortcut `translateX(4px)` on opacity-0 badges).
**Pattern:** Chrome includes the bounding box of CSS transforms in a parent's `scrollWidth` even when the transformed element is `opacity: 0` or otherwise visually hidden. This produces phantom horizontal overflow that the audit catches but is invisible to the user.
**Action:** Code review pass across `app/src/styles/` and any inline styles for:
- `transform: translateX(...)` or `translateY(...)` on elements that are also `opacity: 0`, `visibility: hidden`, or inside an `overflow: hidden` parent that should be `overflow: clip`
- Rotating animations on absolutely-positioned children inside non-clip parents
**Likely fix shape:** Change parent `overflow: hidden` → `overflow: clip` where the parent is meant to be a hard clip container, or move transforms onto pseudo-elements with `inset: 0` containment.

---

## Phase 2C reminder

Phase 2C is the full Tauri runtime audit — `cargo tauri dev` build, audit each page in real backend state. This will surface bugs that Phase 1 (Vite/demo mode) cannot see. Do not start until:
- Phase 2B is complete (or at least P1 items resolved)
- Audit script change-detection is improved (item 6 above)
- A clean baseline is established on `cargo tauri dev`

---

## Reclassified as cosmetic (not user-visible bugs)

### living-background__aura-clip + holo-panel__refraction internal overflow
**Status:** RECLASSIFIED P1 → P3 (cosmetic)
**Verified:** 2026-04-11
**Replaces:** former Batch 2 of api-client P1 work

DOM measurements on /api-client, /dashboard, and /files at viewport
1377x722 confirmed `documentElement.scrollWidth - clientWidth === 0`
and `body.scrollWidth - clientWidth === 0` on all three pages. The
page itself does not scroll horizontally.

The `living-background__aura-clip` (+30 to +44px) and
`holo-panel__refraction` (+180 to +363px) overflows reported by the
audit are real `scrollWidth > clientWidth` measurements, but they
exist *inside clipping containers* — `.living-background` has
`overflow: clip` (per dashboard-01 fix) and `.holo-panel` has
`overflow: clip` (per Phase 2A Cluster B fix). Neither overflow
propagates to a user-visible scroll position. They are by-design
decorative bleed:

- `aura-clip` wraps an aura element scaled to `1.04` for blur edge
  compensation during parallax — load-bearing visual safety margin,
  preserved by dashboard-01 fix
- `holo-panel__refraction` is a rotating conic-gradient pseudo-
  element used as a visual effect; its bounding box exceeds its
  parent by design, clipped by the parent's `overflow: clip`

Magnitudes vary across pages (aura-clip: 30 vs 44, refraction: 363
vs 180) because the inner content widths of the containing elements
vary by page layout. This is expected and not a regression.

**Action:** No code change. The audit script change in the next
commit will reclassify these as P3 cosmetic going forward so future
audits do not surface them as P1.

## Phase 2B P2 Re-Audit Results (2026-04-12)

### Method
Re-audited 4 hardest-hit pages using fixed audit script:
- 9741c5cf: MutationObserver captures transient DOM additions (eliminates Cluster E false positives)
- c56498e9: overflow classification walks ancestor chain for overflow:hidden|clip|scroll|auto (separates user-visible P1 from internal/cosmetic P3)
- Live Puppeteer runs at 1920x1080, 1280x800, 1024x768

### Prior fixes that resolved original P1s
- d0bb2f51: RequiresLlm #/settings anchor → history.pushState (fixes cs-04, md-06 + 12 other gated pages)
- fdeaa172: showDemoToast() verified working (fixes gate button feedback)
- 743bde12: living-background__aura overflow:clip container (fixes all background overflow findings)

### Reclassification table

| Page | Finding | Original | New | Reason |
|------|---------|----------|-----|--------|
| governed-control | gc-01 background overflow | P1 | FP | documentElement.scrollWidth === clientWidth at all viewports; clipped by ancestor |
| governed-control | gc-02 holo-panel overflow | P1 | P3 | decorative bleed clipped by overflow:clip; by-design |
| governed-control | gc-03 empty agent select | P1 | P3 | real demo-mode gap but not user-blocking; always been this way |
| governed-control | gc-04 missing aria-label | P1 | P3 | real a11y gap, low effort fix, not blocking |
| world-sim | ws-01 background overflow | P1 | FP | same as gc-01 |
| world-sim | ws-02 holo-panel overflow | P1 | P3 | same as gc-02 |
| world-sim | ws-03 duplicate H1 | P1 | P3 | real semantic issue, shell+page H1 pattern, not blocking |
| world-sim | ws-04 empty agent select | P1 | P3 | same as gc-03 |
| consciousness | cs-01 background overflow | P1 | FP | same as gc-01 |
| consciousness | cs-02 holo-panel overflow | P1 | P3 | same as gc-02 |
| consciousness | cs-03 gate buttons silent | P1 | FP | MutationObserver now captures Refresh text change + Start Jarvis error badge; only Install Ollama remains dead (P3) |
| consciousness | cs-04 #/settings broken | P1 | FP | fixed by d0bb2f51; live re-audit confirms navigation works |
| media | md-01 background overflow | P1 | FP | same as gc-01 |
| media | md-03 Refresh silent | P1 | FP | MutationObserver captures innerText change |
| media | md-04 Start Jarvis silent | P1 | FP | MutationObserver captures error badge |
| media | md-05 Install Ollama silent | P1 | P3 | real but downgraded; zero feedback in demo mode only |
| media | md-06 #/settings broken | P1 | FP | fixed by d0bb2f51 |

### Summary

| Page | Original P1 | Surviving P1 | → P3 | → FP |
|------|------------|-------------|------|------|
| governed-control | 4 | 0 | 3 | 1 |
| world-sim | 4 | 0 | 3 | 1 |
| consciousness | 4 | 0 | 1 | 3 |
| media | 5 | 0 | 1 | 4 |
| **TOTAL** | **17** | **0** | **8** | **9** |

### Remaining P3 backlog (not blocking Phase 2C)
- Install Ollama button: zero feedback in demo mode (gc-03, md-05)
- Empty agent selects in demo mode (gc-03, ws-04)
- Missing aria-label on agent selects (gc-04)
- Duplicate H1 shell+page pattern (ws-03, likely affects other pages)
- Holo-panel decorative overflow internal clipping (gc-02, ws-02, cs-02, md-01)

### Phase 2B status: COMPLETE
P1 tier: 11 commits, all CI-green (276b00dd)
P2 tier: 17 → 0 P1s after re-audit with fixed tooling
Infrastructure: audit script false positive classes eliminated (MutationObserver + overflow classification)
LLM batch: 4 commits landed (e57c5e06, 3cb003bd, 748d99e8, 2b812619)

Next: Phase 2C — full Tauri runtime audit with cargo tauri dev

