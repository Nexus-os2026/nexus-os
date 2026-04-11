# Audit: Permissions
URL: http://localhost:1420/permissions
Audited at: 2026-04-09T21:42:00Z
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
none (on initial load)

Note: Errors triggered by user interaction are documented in Click Sequence below.

### Warnings
none

### Logs
none

### Info
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js:21550

### Debug
- `[vite] connecting...` — @vite/client:494
- `[vite] connected.` — @vite/client:617

## Overflow

### 1920x1080 (effective viewport 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px, clipped by overflow:hidden]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]
  - `button` (x2): scrollWidth=56/50 clientWidth=32 [OVERFLOW, icon buttons clipped]

### 1280x800 (simulated 1248x615)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK — viewport did not resize; simulated via container constraint]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main: not re-measured (viewport stuck at 1888; container simulation used)
- other overflowing:
  - `div.living-background`: scrollWidth=1344 clientWidth=1248 [OVERFLOW +96px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px, clipped]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]
  - `button` (x2): scrollWidth=56/50 clientWidth=32 [OVERFLOW, icon buttons clipped]

### 1024x768 (simulated 992x583)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK — simulated]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main: not re-measured (simulated)
- other overflowing:
  - `div.living-background`: scrollWidth=1344 clientWidth=1248 [OVERFLOW +96px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=724 clientWidth=666 [OVERFLOW +58px, clipped]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]
  - `button` (x2): scrollWidth=56/50 clientWidth=32 [OVERFLOW, icon buttons clipped]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | ← Back | button | — | yes |
| 2 | × | button | — | yes |
| 3 | Revoke All Network | button | — | yes |
| 4 | Read-Only Mode | button | — | yes |
| 5 | Minimal Mode | button | — | yes |
| 6 | (model routing select) | select | — | yes |
| 7 | Local only (no cloud) | input[checkbox] | — | yes |
| 8 | ▶ Permission History (0) | button | — | yes |

## Click sequence
### Click 1: "← Back"
- Pathname before: /permissions
- New console: clean
- Network failures: none
- Visible change: Navigated to /agents page
- Pathname after: /agents (confirmed via tab URL)
- Reverted: yes (navigated back to /permissions)

### Click 2: "×"
- Pathname before: /permissions
- New console: clean
- Network failures: none
- Visible change: Dismissed the agent-specific permission banner/card containing the × button; the select and checkbox elements also disappeared from the DOM. Panel and remaining buttons persisted.
- Pathname after: /permissions
- Reverted: yes (reloaded /permissions to restore full UI)

### Click 3: "Read-Only Mode"
- Pathname before: /permissions
- New console: clean
- Network failures: none
- Visible change: No visible change. Button has no active/pressed state, no aria-pressed attribute, no class change. Click appears to have no effect in demo mode.
- Pathname after: /permissions
- Reverted: n/a

### Click 4: "Minimal Mode"
- Pathname before: /permissions
- New console: clean
- Network failures: none
- Visible change: No visible change. Same as Read-Only Mode — no visual feedback.
- Pathname after: /permissions
- Reverted: n/a

### Click 5: select (changed to "Ollama (local)")
- Pathname before: /permissions
- New console: **ERROR** `[Nexus OS] Unhandled promise rejection: Error: desktop runtime unavailable` at main.tsx:41 → invokeDesktop (backend.ts:17) → setAgentLlmProvider (backend.ts:519) → onChange (PermissionDashboard.tsx:706)
- Network failures: none (Tauri invoke, not HTTP)
- Visible change: Select value changed to "ollama" visually, but the backend call failed.
- Pathname after: /permissions
- Reverted: n/a

### Click 6: checkbox "Local only (no cloud)"
- Pathname before: /permissions
- New console: **ERROR** `[Nexus OS] Unhandled promise rejection: Error: desktop runtime unavailable` at main.tsx:41 → invokeDesktop (backend.ts:17) → setAgentLlmProvider (backend.ts:519) → onChange (PermissionDashboard.tsx:763)
- Network failures: none (Tauri invoke, not HTTP)
- Visible change: Checkbox toggled from unchecked to checked visually, but backend call failed.
- Pathname after: /permissions
- Reverted: n/a

### Click 7: "▶ Permission History (0)"
- Pathname before: /permissions
- New console: clean
- Network failures: none
- Visible change: Button text changed from "▶ Permission History (0)" to "▼ Permission History (0)". Expanded section revealed category filter buttons: All Categories, Filesystem, Network, AI / LLM, System, Social Media, Messaging.
- Pathname after: /permissions
- Reverted: n/a

### Skipped (destructive)
- "Revoke All Network" — reason: skipped: destructive keyword "revoke"

### Total interactive elements found: 8
### Elements clicked: 7 (1 skipped as destructive)

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `select[type=select-one]` — no id, no aria-label, no aria-labelledby, not wrapped in label)
- Buttons without accessible name: 0
- Additional: Checkbox is wrapped in `<label>` with text "Local only (no cloud)" — accessible.

## Findings

### permissions-01
- SEVERITY: P1
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: Changing the model routing `<select>` fires `onChange` at PermissionDashboard.tsx:706 which calls `setAgentLlmProvider` → `invokeDesktop` (backend.ts:17). In demo mode this throws `Error: desktop runtime unavailable`, surfaced as unhandled promise rejection at main.tsx:41. Same error on checkbox toggle at PermissionDashboard.tsx:763.
- IMPACT: Two interactive controls (select + checkbox) produce unhandled console errors in demo mode. No user-facing error message or graceful fallback — the controls appear to work but silently fail.

### permissions-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` scrollWidth exceeds clientWidth at every viewport: 2039 vs 1888 (+151px) at 1920; 1344 vs 1248 (+96px) at 1280/1024. This is a decorative background layer that overflows the viewport.
- IMPACT: Cosmetic overflow from background animation element. Not user-visible due to body not scrolling, but contributes to incorrect layout measurements.

### permissions-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` scrollWidth exceeds clientWidth at every viewport: 1716 vs 1577 (+139px) at 1920; 1019 vs 937 (+82px) at 1280; 724 vs 666 (+58px) at 1024. Content is clipped by `overflow: hidden`.
- IMPACT: Panel content extends 58-139px beyond visible area and is silently clipped. Content at the right edge may be invisible to users.

### permissions-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The model routing `<select>` element has no `id`, no `aria-label`, no `aria-labelledby`, and is not wrapped in a `<label>` element. Screen readers cannot identify its purpose.
- IMPACT: Unlabeled form control — screen reader users cannot determine this select controls the LLM routing strategy.

### permissions-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<h1>Permissions</h1>` is located in the shell header (`div.flex`), outside `<main class="nexus-shell-content">`. The `<main>` landmark does not contain the page heading.
- IMPACT: Screen reader users navigating by landmark will not find the page heading inside the main content region.

### permissions-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Read-Only Mode" and "Minimal Mode" buttons (classes `perm-bulk-btn perm-bulk-readonly` / `perm-bulk-minimal`) produce no visible change on click. No class toggle, no `aria-pressed`, no content change, no console output. Buttons appear completely non-functional in demo mode.
- IMPACT: Two action buttons give no feedback — users cannot tell whether the action succeeded, failed, or is gated behind the Tauri backend.

### permissions-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 6 buttons inside `<main>` lack an explicit `type` attribute: "← Back", "×", "Revoke All Network", "Read-Only Mode", "Minimal Mode", "▶ Permission History (0)". Additionally, 2 shell header buttons ("Refresh", "Start Jarvis") also lack `type`. Browsers default `<button>` to `type="submit"`, which can cause unintended form submissions.
- IMPACT: 8 buttons total default to `type="submit"` instead of `type="button"`, risking unintended form submission if any are placed inside a `<form>`.

## Summary
- Gate detected: no
- Total interactive elements: 8
- Elements clicked: 7 (1 skipped as destructive)
- P0: 0
- P1: 1
- P2: 6
