# Audit: Compliance
URL: http://localhost:1420/compliance
Audited at: 2026-04-09T21:30:00+01:00
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
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px, hidden by overflow:hidden]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px, hidden by overflow:hidden]

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px, hidden by overflow:hidden]

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1065 clientWidth=992 [OVERFLOW +73px, hidden by overflow:hidden]

## Interactive elements (main content only)

### Tab buttons (always visible)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Overview | button | — | yes |
| 2 | Risk Cards | button | — | yes |
| 3 | SOC 2 | button | — | yes |
| 4 | Chain | button | — | yes |
| 5 | Governance | button | — | yes |
| 6 | Security | button | — | yes |
| 7 | Reports | button | — | yes |
| 8 | Erasure | button | — | yes |
| 9 | Provenance | button | — | yes |
| 10 | Retention | button | — | yes |

### Sub-tab content elements (visible when parent tab active)
| # | Label | Type | Parent tab | Enabled |
|---|-------|------|------------|---------|
| 11 | Verify Chain Now | button (.cd-generate-btn) | Chain | yes |
| 12 | (time filter select) | select (.cd-time-select) | Governance | yes |
| 13 | (time filter select) | select (.cd-time-select) | Security | yes |
| 14 | Run Retention Enforcement | button (.cd-generate-btn) | Retention | yes |

## Click sequence

### Click 1: "Overview" (tab)
- Pathname before: /compliance
- New console: clean
- Network failures: none
- Visible change: none (already active on load)
- Pathname after: /compliance
- Reverted: n/a

### Click 2: "Risk Cards" (tab)
- Pathname before: /compliance
- New console: clean
- Network failures: none
- Visible change: content switches to "Per-Agent EU AI Act Risk Classification — No agents registered — create agents to see risk classification."
- Pathname after: /compliance
- Reverted: n/a

### Click 3: "SOC 2" (tab)
- Pathname before: /compliance
- New console: clean
- Network failures: none
- Visible change: content switches to "SOC 2 Type II Compliance Controls — Real-time SOC 2 control status from Nexus OS governance primitives. No SOC 2 controls evaluated — ensure agents are registered."
- Pathname after: /compliance
- Reverted: n/a

### Click 4: "Chain" (tab)
- Pathname before: /compliance
- New console: clean
- Network failures: none
- Visible change: content switches to "Hash Chain Verification — Verify the integrity of the append-only audit trail by checking every hash link in the chain." with "VERIFY CHAIN NOW" button
- Pathname after: /compliance
- Reverted: n/a

### Click 5: "Governance" (tab)
- Pathname before: /compliance
- New console: clean
- Network failures: none
- Visible change: content switches to "Governance Metrics" with time filter select (Last Hour / Last 24 Hours / Last 7 Days / Last 30 Days / All Time). Shows "No governance metrics available."
- Pathname after: /compliance
- Reverted: n/a

### Click 6: "Security" (tab)
- Pathname before: /compliance
- New console: clean
- Network failures: none
- Visible change: content switches to "Security Events" with time filter select. Shows "No security events in the selected time range."
- Pathname after: /compliance
- Reverted: n/a

### Click 7: "Reports" (tab)
- Pathname before: /compliance
- New console: clean
- Network failures: none
- Visible change: content switches to "Transparency Report Viewer — Select an agent to generate an EU AI Act Article 13 transparency report. No agents registered."
- Pathname after: /compliance
- Reverted: n/a

### Click 8: "Erasure" (tab)
- Pathname before: /compliance
- New console: clean
- Network failures: none
- Visible change: content switches to "Cryptographic Erasure (GDPR Article 17) — Trigger complete agent data erasure: audit events redacted, encryption keys destroyed, identity purged. No agents registered."
- Pathname after: /compliance
- Reverted: n/a

### Click 9: "Provenance" (tab)
- Pathname before: /compliance
- New console: clean
- Network failures: none
- Visible change: content switches to "Data Provenance & Lineage — Track data origin, transformations, and flow through agents. No audit events recorded yet — provenance data will appear as agents operate."
- Pathname after: /compliance
- Reverted: n/a

### Click 10: "Retention" (tab)
- Pathname before: /compliance
- New console: clean
- Network failures: none
- Visible change: content switches to "Retention Policy Settings — Configure data retention periods per data class. Events beyond the retention period are purged (redacted) during enforcement." Shows retention periods and "Run Retention Enforcement" button.
- Pathname after: /compliance
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 14
### Elements clicked: 10 (capped at 10 — all 10 tab buttons clicked; sub-tab elements not clicked in this pass)

## Accessibility
- Images without alt: 0
- Inputs without label: 2 (selectors: `select.cd-time-select` in Governance tab, `select.cd-time-select` in Security tab — both lack id, aria-label, aria-labelledby, and wrapping label)
- Buttons without accessible name: 0
- Tab buttons lack `role="tab"` and `aria-selected` attributes
- Tab container `nav` lacks `role="tablist"`
- H1 "Compliance" is outside `<main>` element (in shell header `div.flex.flex-wrap.items-center.gap-2.5`)

## Findings

### compliance-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` has scrollWidth exceeding clientWidth at all three viewports: 2039 vs 1888 (+151px) at 1920x1080, 1348 vs 1248 (+100px) at 1280x800, 1065 vs 992 (+73px) at 1024x768. Clipped by `overflow:hidden` so no visible scrollbar, but the element is wider than the viewport.
- IMPACT: Background element exceeds viewport width; cosmetic issue since overflow is hidden but indicates incorrect sizing.

### compliance-02
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Both `select.cd-time-select` elements (Governance tab and Security tab) have no `id`, no `aria-label`, no `aria-labelledby`, and no associated `<label>` element. Screen readers cannot identify the purpose of these controls.
- IMPACT: Time filter selects are inaccessible to assistive technology users.

### compliance-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The 10 tab buttons (`button.cd-tab`) lack `role="tab"` and `aria-selected` attributes. The containing `<nav>` element lacks `role="tablist"`. The tab pattern does not conform to WAI-ARIA Tabs pattern.
- IMPACT: Assistive technology cannot convey tab navigation semantics; users cannot determine which tab is active or how many tabs exist.

### compliance-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<h1>Compliance</h1>` is located in the shell header (`div.flex.flex-wrap.items-center.gap-2.5`), outside the `<main>` landmark element.
- IMPACT: Screen reader users navigating by landmarks will not find the page heading within the main content region.

### compliance-05
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has scrollWidth=1716 vs clientWidth=1577 (+139px overflow) at 1920x1080 viewport. Clipped by `overflow:hidden`.
- IMPACT: Main content panel is wider than its container; content may be clipped at the right edge.

## Summary
- Gate detected: no
- Total interactive elements: 14
- Elements clicked: 10
- P0: 0
- P1: 0
- P2: 5
