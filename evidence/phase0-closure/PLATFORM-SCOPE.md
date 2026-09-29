# Platform scope (Owner decision) and deferred Windows/macOS findings

Owner decision, 2026-09-29: the Phase Zero active target is **Linux only**.
Windows and macOS work is deferred to future dedicated portability stages.
Windows/macOS code and tests are preserved (not deleted or weakened), and
the Windows/macOS jobs and harness steps remain in `ci.yml`; no further
hosted cross-platform run is dispatched in this stage.

## What the one hosted run showed (run `36559976709`, #109, commit `9ed607f8`)

- test-linux, test-frontend, test-python: **success**.
- test-macos: **compile error** in the desktop crate's lib-test build:
  `symbol _EMBED_INFO_PLIST is already defined`. The desktop tests expand
  `tauri::generate_context!()` a second time (item D ACL test and app-origin
  test in `phase0_surface/fg_webview/tests.rs`), and on macOS dev builds each
  expansion embeds an Info.plist symbol. tauri-codegen 2.5.5 skips that embed
  when `generate_context!(test = true)` is used; the live harness
  (`tests/webview_boundary_live.rs`) likely needs the same because it links
  the desktop library. Nothing ran on macOS.
- test-windows: desktop lib tests 439 passed, 1 failed:
  `p0_fg_webview_boundary_exposes_only_build_main_window` matches the
  two-line pattern `#[doc(hidden)]\npub mod webview_boundary;` in `lib.rs`
  read through `include_str!` without CRLF normalisation (Windows checkout
  with autocrlf). `cargo test --workspace` stopped there, so about 130 test
  binaries, the live harness and the packaged-toolchain steps did not run on
  Windows.

## Deferred candidate fixes (not committed)

Prepared, then reverted under the Linux-only decision:
1. `tauri::generate_context!(test = true)` at the two test call sites in
   `fg_webview/tests.rs` and in `tests/webview_boundary_live.rs`.
2. `strip_rust_comments` in `fg_webview/tests.rs` normalises `\r\n` to `\n`
   before stripping.

## Portability audits (read-only)

- Audit A (all crates except the desktop app): no will-fail or likely-fail
  item found; low risks only (Windows/macOS-only cfg code compiled by those
  lanes only; aws-lc-sys 0.45 on macOS unproven; a canonicalised Windows path
  join that is correct; loopback timing margins).
- Audit B (desktop app and live harness): stopped under the Linux-only
  decision before reporting; no findings recorded.
- Not verified on Windows or macOS: the live webview harness checks (Windows
  raw-postMessage subframe path, WebView2 redirect events, macOS behaviour),
  key-file validation on macOS, and the new tests in the crates that did not
  run.

```diff
diff --git a/app/src-tauri/src/phase0_surface/fg_webview/tests.rs b/app/src-tauri/src/phase0_surface/fg_webview/tests.rs
index 2b62ef5e..42df00b9 100644
--- a/app/src-tauri/src/phase0_surface/fg_webview/tests.rs
+++ b/app/src-tauri/src/phase0_surface/fg_webview/tests.rs
@@ -37,6 +37,9 @@ fn strip_rust_comments(src: &str) -> String {
     fn ident(b: u8) -> bool {
         b.is_ascii_alphanumeric() || b == b'_'
     }
+    // A Windows checkout may carry CRLF line endings; matches that span a
+    // line use "\n".
+    let src = src.replace("\r\n", "\n");
     let b = src.as_bytes();
     let mut out: Vec<u8> = Vec::with_capacity(b.len());
     let mut i = 0;
@@ -516,7 +519,7 @@ fn p0_fg_webview_app_origin_is_resolved_like_tauri_resolves_the_app_url() {
     use tauri::utils::config::FrontendDist;
     use tauri::Url;
 
-    let ctx: tauri::Context<tauri::Wry> = tauri::generate_context!();
+    let ctx: tauri::Context<tauri::Wry> = tauri::generate_context!(test = true);
     let config = ctx.config().clone();
     let main = config
         .app
@@ -877,7 +880,7 @@ fn p0_fg_webview_boundary_exposes_only_build_main_window() {
 fn p0_fg_webview_app_command_ipc_is_local_main_only() {
     use tauri::ipc::Origin;
 
-    let mut ctx: tauri::Context<tauri::Wry> = tauri::generate_context!();
+    let mut ctx: tauri::Context<tauri::Wry> = tauri::generate_context!(test = true);
     let authority = ctx.runtime_authority_mut();
 
     let remote = Origin::Remote {
diff --git a/app/src-tauri/tests/webview_boundary_live.rs b/app/src-tauri/tests/webview_boundary_live.rs
index 13be99eb..4d0ddaf6 100644
--- a/app/src-tauri/tests/webview_boundary_live.rs
+++ b/app/src-tauri/tests/webview_boundary_live.rs
@@ -869,7 +869,7 @@ fn run_live(mode: Mode, phase: &Mutex<String>, servers: &Servers) -> Vec<String>
     let page_loads: PageLoads = Arc::default();
 
     set_phase(phase, "build");
-    let mut ctx: tauri::Context<tauri::Wry> = tauri::generate_context!();
+    let mut ctx: tauri::Context<tauri::Wry> = tauri::generate_context!(test = true);
     let expected_origin = match mode {
         Mode::DevOrigin => "http://localhost:1420",
         Mode::AppOrigin if cfg!(windows) => "http://tauri.localhost",
```
