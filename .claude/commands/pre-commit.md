Use the approved Mother mission's exact checks and shared Cargo slot first.
Inspect status, complete relevant diff, dependencies, secrets and scope; run
git diff --check. Run formatting, applicable Clippy and meaningful targeted
Rust/native GTK4 tests for affected behavior. Run a full-workspace or live host
gate only when required or justified by new evidence. Never invoke deleted app
builds. Report each actual command and result, plus omissions and blockers.
Commit only if authorized, staging explicit files; no push or integration is
implied. Local green is not CI or physical acceptance.
