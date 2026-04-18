//! Oracle runtime status command.
//!
//! Single Tauri command that surfaces the
//! [`crate::oracle_runtime::OracleRuntime`] health to the frontend. Added
//! in Phase 1.5a alongside the production oracle wiring.

use crate::oracle_runtime::OracleRuntimeStatus;
use crate::AppState;

#[tauri::command]
pub async fn oracle_runtime_status(
    state: tauri::State<'_, AppState>,
) -> Result<OracleRuntimeStatus, String> {
    Ok(state.oracle_runtime_status())
}
