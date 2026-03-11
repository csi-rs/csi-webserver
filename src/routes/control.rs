use axum::{Json, extract::State, http::StatusCode};

use crate::{
    models::{ApiResponse, StartConfig},
    state::AppState,
};

// ─── POST /api/control/start ────────────────────────────────────────────────

/// Start CSI data collection.
///
/// Always re-enforces `array-list` log mode before issuing the `start`
/// command so the serial task receives parseable output regardless of
/// any prior configuration.
///
/// Body (all fields optional):
/// ```json
/// { "duration": 120 }   // omit for indefinite collection
/// ```
pub async fn start_collection(
    State(state): State<AppState>,
    body: Option<Json<StartConfig>>,
) -> (StatusCode, Json<ApiResponse>) {
    // Enforce array-list mode before every start.
    if let Err(e) = state
        .cmd_tx
        .send("set-log-mode --mode=array-list".to_string())
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Failed to enforce log mode: {e}"),
            }),
        );
    }

    let cmd = body
        .map(|Json(b)| b.to_cli_command())
        .unwrap_or_else(|| "start".to_string());

    match state.cmd_tx.send(cmd.clone()).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: format!("Collection started: {cmd}"),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Failed to start collection: {e}"),
            }),
        ),
    }
}
