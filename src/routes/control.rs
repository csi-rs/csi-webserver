use axum::{Json, extract::State, http::StatusCode};
use chrono::Local;

use crate::{
    models::{ApiResponse, StartConfig},
    state::AppState,
};

// ─── POST /api/control/start ────────────────────────────────────────────────

/// Start CSI data collection.
///
/// Body (all fields optional):
/// ```json
/// { "duration": 120 }   // omit for indefinite collection
/// ```
pub async fn start_collection(
    State(state): State<AppState>,
    body: Option<Json<StartConfig>>,
) -> (StatusCode, Json<ApiResponse>) {
    let cmd = body
        .map(|Json(b)| b.to_cli_command())
        .unwrap_or_else(|| "start".to_string());

    match state.cmd_tx.send(cmd.clone()).await {
        Ok(_) => {
            // Generate a timestamped session dump file path and notify the
            // serial task. The file is only opened if the output mode includes
            // Dump; otherwise the path is remembered and used if the mode
            // switches later during the same session.
            let path = format!(
                "csi_dump_{}.bin",
                Local::now().format("%Y%m%d_%H%M%S")
            );
            tracing::info!("New session dump file: {path}");
            let _ = state.session_file_tx.send(Some(path));

            (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    message: format!("Collection started: {cmd}"),
                }),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Failed to start collection: {e}"),
            }),
        ),
    }
}
