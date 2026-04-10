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
