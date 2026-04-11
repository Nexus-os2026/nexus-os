# Nexus OS Phase 1 Triage Plan

**Generated:** 2026-04-10
**Source:** `docs/page-audits/phase1-vite/_MASTER_FINDINGS.md` + per-page markdowns
**Audit corpus:** 88 routes, 0 P0, 146 P1, 509 P2 (655 total findings)
**Triage method:** Cluster by root cause across the corpus, fix shared causes once.

---

## Top-line numbers

| Severity | Count |
|---|---|
| P0 (broken) | 0 |
| P1 (degraded UX) | 146 |
| P2 (polish) | 509 |
| **Total** | **655** |

**Pages with zero P1s:** 19 (~22% of surface is already healthy in demo mode).
**Hardest-hit pages (P1 ≥ 4):** api-client, governed-control, world-sim, consciousness, media.

---

## Cluster identification

Each cluster represents a single root cause that produces findings on multiple pages. The cluster fix is applied once; verification confirms the blast radius across the whole corpus.

| ID | Cluster | Pages affected | Estimated findings closed | Root cause |
|---|---|---|---|---|
| **A** | `div.living-background` width overflow | 88 / 88 | 88 | Decorative shell background ~147–151px wider than viewport at all breakpoints. Fixed-position, clipped, but structurally wrong on every page. |
| **B** | `section.holo-panel.holo-panel-mid` clips children | 88 / 88 | 88 | Shared holo-panel container has `overflow:hidden` and inner content exceeds container width. Severity varies: cosmetic where decorative, P1 where it hides functional content. |
| **C** | `span.nexus-sidebar-item-text` 4px clipping | ≥50 / 88 (likely all) | ≥50, probably 88 | Sidebar item text has scrollWidth=157 vs clientWidth=153 — text wider than its container by 4px. Last few pixels of some nav labels clipped without ellipsis. Audit didn't always re-measure sidebar — true blast radius is probably 88. |
| **D** | `type="submit"` on standalone buttons | 61 / 88 | 61 (and possibly some of E) | Buttons in shell header (Refresh, Start Jarvis) and many page-level buttons declared `type="submit"` but not inside any `<form>`. Semantically wrong; **may be silently swallowing click handlers** → may explain a fraction of cluster E. |
| **E** | Refresh / Start Jarvis silently no-op in demo mode | 67 / 88 | 67 (minus whatever D fixes) | Shell-level header buttons require `hasDesktopRuntime()`, no user feedback when blocked. Should show a "demo mode" toast or disable with explanation. |
| **F** | Icon buttons use `title` instead of `aria-label` | 6 / 88 | 6 | Small a11y issue. `title` is less reliable than `aria-label` for screen readers. |
| **G** | React StrictMode double-invoke noise | 18 / 88 | 0 (not a bug) | React intentional double-mount in dev mode. Auditor flagged as console noise but it's expected behavior. **Ignore.** |
| **H** | Leaked `[TEST]` debug code in console | 4 / 88 | 4 | A `console.log("[TEST] attaching standalone test listener")` and an unguarded `listen()` call in `Agents.tsx:200-204` and 3 similar files. Test code that shipped to the page. |

### Coverage math

Conservatively, 7 cluster fixes (A, B, C, D, E, F, H — skipping G as not-a-bug) close approximately:

- **88 + 88 + 50 + 61 + 67 + 6 + 4 = 364 findings**
- **= ~56% of all 655 findings**
- **= ~85% of P2 findings + a meaningful chunk of P1**

Remaining ~290 findings are page-specific bugs requiring per-page attention. Most of those need Phase 2 (Tauri runtime) to fully resolve because they involve real backend behavior.

---

## Fix order

The order matters. Three principles drive it:

1. **Quick wins first** to clear noise from later audits and build momentum.
2. **Cluster D before E** — there's a hypothesis that some "dead button" findings in cluster E are actually `type="submit"` swallowing click handlers (cluster D in disguise). Fixing D first lets us measure how much of E was actually D.
3. **Each cluster fix is followed by verification** — re-audit 3 sample pages (Dashboard, Files, API Client) to confirm the fix works without regressions before moving on.

### Phase 2A — Cleanup (this weekend, ~3–4 hours)

| Step | Cluster | Action | Verification | Time |
|---|---|---|---|---|
| 1 | A | Fix `div.living-background` width in shared shell CSS | Re-audit Dashboard, Files, API Client; confirm `living-background` no longer in overflow lists | 20 min |
| 2 | B | Fix `holo-panel-mid` overflow handling in shared component | Same 3 pages; confirm `holo-panel` no longer in overflow lists | 30 min |
| 3 | H | Remove `[TEST]` debug listener from `Agents.tsx:200-204` and 3 similar files | Re-audit Agents; confirm console errors gone | 15 min |
| 4 | F | Replace `title` with `aria-label` on 6 affected icon buttons | Re-audit affected pages | 15 min |
| 5 | **D experiment** | Fix `type="submit"` → `type="button"` on **Dashboard only**, re-audit, compare dead-button counts | If dead-button count drops → D and E are linked. If not → independent. | 45 min |
| 6 | C | Fix `span.nexus-sidebar-item-text` width / overflow in sidebar component | Re-audit any page; confirm sidebar text overflow gone | 20 min |
| 7 | D (full) | Apply `type="submit"` → `type="button"` rollout across all 61 affected pages | Re-audit Dashboard + 2 others | 60 min |
| 8 | E | Add demo-mode feedback (toast or disabled state) to shell-level Refresh / Start Jarvis | Re-audit Dashboard, Files, API Client | 45 min |

**End state of Phase 2A:** ~285 P2 findings closed, ~25–40 P1 findings closed (the ones that overlap with clusters), corpus drops from 655 → ~290 findings.

### Phase 2B — Page-specific bugs (next week)

After Phase 2A, re-run the audit driver:

```bash
cd ~/NEXUS/nexus-os/scripts/page-audit
FORCE_RESET=1 bash run_batch.sh
```

This produces a new master findings file with the cluster bugs gone. The remaining ~290 findings are real per-page bugs. Group them by feature area (admin pages, agent-lab pages, measurement pages, etc.) and fix one cluster per Claude Code session.

Hardest-hit pages to prioritize in Phase 2B:
- api-client (sidebar gets stuck in Audit view, real state-management bug)
- governed-control (P1=4)
- world-sim (P1=4)
- consciousness (P1=4)
- media (P1=5)

### Phase 2C — Phase 2 audit (real Tauri runtime)

After Phase 2A and 2B land, the demo-mode audit has done all it can. Move to Phase 2: build the Tauri app in dev mode, audit each page in its real runtime state, catch the bugs that only surface with backend behavior. This is a separate setup task — `tauri-driver` + WebDriver, or computer-use API driving the desktop window.

**Do not start Phase 2C until Phase 2A is complete.** The cluster bugs would pollute Phase 2 findings.

---

## Verification protocol (applies to every cluster fix)

Each cluster fix follows the same shape:

1. **Identify the file(s)** to change. For shared-component clusters (A, B, C, E), this is one file. For search-and-replace clusters (D, F, H), this is multiple files identified by grep.
2. **Make the change.** Smallest possible diff. Don't refactor neighboring code.
3. **Run the standard checks** on touched crates / packages only:
   ```bash
   pnpm lint && pnpm test
   ```
   For Rust changes (none expected in Phase 2A but possible later):
   ```bash
   cargo fmt -p <crate> && cargo clippy -p <crate> -- -D warnings && cargo test -p <crate>
   ```
4. **Visual smoke test in Vite** at `localhost:1420` — load 2–3 affected pages manually, eyeball the fix.
5. **Targeted re-audit** of 3 sample pages (Dashboard, Files, API Client). If the cluster's findings disappear from those 3 pages, the fix is good.
6. **Commit** with message format: `fix(audit-cluster-X): <one-line description> — closes ~N findings across N pages`.
7. **Update tracker** at `docs/page-audits/phase1-vite/_TRIAGE_PROGRESS.md` (created on first fix).

---

## Don't-do list

- **Don't fix pages one at a time.** The whole point of triage is leverage. Fix root causes once.
- **Don't open all 88 markdowns.** Use grep against the corpus. The tools are faster than your eyes.
- **Don't fix cluster G.** React StrictMode is intentional dev-mode behavior. Production builds don't have it.
- **Don't start Phase 2B before 2A is complete.** Cluster fixes change the noise floor; per-page work is easier with quiet pages.
- **Don't run a full re-audit between every step.** Targeted re-audits on 3 sample pages are enough until end of Phase 2A.
- **Don't skip the Cluster D experiment** (step 5). Knowing whether D and E are linked changes how you write the E fix.

---

## Triage commitment

You committed to triage within 48 hours of Phase 1 completing. Phase 1 finished 2026-04-10 02:15. **Triage deadline: 2026-04-12 02:15.** This document satisfies the "plan written" half of the commitment. The "fix campaign queued" half is satisfied by writing the Cluster A fix prompt and putting it in front of Claude Code today.

---

## Next action

Run the Cluster A fix prompt (in `prompts/triage/cluster-a-living-background.txt`) through Claude Code. Verify on the 3 sample pages. Commit. Move to Cluster B.
