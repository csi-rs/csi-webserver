use axum::{Json, extract::State, http::StatusCode};

use crate::{
    models::{
        ApiResponse, CollectionModeConfig, CsiConfig, DeviceConfig, TrafficConfig, WifiConfig,
    },
    state::AppState,
};

// ─── GET /api/config ────────────────────────────────────────────────────────

/// Return the server-side cached device configuration as JSON.
pub async fn get_config(State(state): State<AppState>) -> Json<DeviceConfig> {
    let config = state.config.lock().await;
    Json(config.clone())
}

// ─── POST /api/config/reset ─────────────────────────────────────────────────

pub async fn reset_config(
    State(state): State<AppState>,
) -> (StatusCode, Json<ApiResponse>) {
    let result = send_cmd(&state, "reset-config".to_string()).await;
    if result.0 == StatusCode::OK {
        *state.config.lock().await = DeviceConfig::default();
    }
    result
}

// ─── POST /api/config/wifi ──────────────────────────────────────────────────

pub async fn set_wifi(
    State(state): State<AppState>,
    Json(body): Json<WifiConfig>,
) -> (StatusCode, Json<ApiResponse>) {
    let cmd = body.to_cli_command();
    let result = send_cmd(&state, cmd).await;
    if result.0 == StatusCode::OK {
        let mut cfg = state.config.lock().await;
        cfg.wifi_mode = Some(body.mode);
        cfg.channel = body.channel;
        cfg.sta_ssid = body.sta_ssid;
    }
    result
}

// ─── POST /api/config/traffic ───────────────────────────────────────────────

pub async fn set_traffic(
    State(state): State<AppState>,
    Json(body): Json<TrafficConfig>,
) -> (StatusCode, Json<ApiResponse>) {
    let cmd = body.to_cli_command();
    let result = send_cmd(&state, cmd).await;
    if result.0 == StatusCode::OK {
        state.config.lock().await.traffic_hz = Some(body.frequency_hz);
    }
    result
}

// ─── POST /api/config/csi ───────────────────────────────────────────────────

pub async fn set_csi(
    State(state): State<AppState>,
    Json(body): Json<CsiConfig>,
) -> (StatusCode, Json<ApiResponse>) {
    send_cmd(&state, body.to_cli_command()).await
}

// ─── POST /api/config/collection-mode ──────────────────────────────────────

pub async fn set_collection_mode(
    State(state): State<AppState>,
    Json(body): Json<CollectionModeConfig>,
) -> (StatusCode, Json<ApiResponse>) {
    let cmd = body.to_cli_command();
    let result = send_cmd(&state, cmd).await;
    if result.0 == StatusCode::OK {
        state.config.lock().await.collection_mode = Some(body.mode);
    }
    result
}

// ─── Shared helper ──────────────────────────────────────────────────────────

async fn send_cmd(state: &AppState, cmd: String) -> (StatusCode, Json<ApiResponse>) {
    match state.cmd_tx.send(cmd.clone()).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: format!("Sent: {cmd}"),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Failed to send command: {e}"),
            }),
        ),
    }
}
