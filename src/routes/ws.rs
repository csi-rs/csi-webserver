use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use tokio::sync::broadcast;

use crate::state::AppState;

// ─── GET /api/ws ────────────────────────────────────────────────────────────

/// Upgrade an HTTP connection to a WebSocket and stream CSI packets as JSON.
///
/// Each message sent to the client is the JSON-serialised form of a
/// `CsiPacket` — one per received CSI frame from the ESP32.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let rx = state.csi_tx.subscribe();
    ws.on_upgrade(|socket| handle_socket(socket, rx))
}

async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    loop {
        tokio::select! {
            // ── Forward CSI JSON to the WebSocket client ──────────────────
            result = rx.recv() => {
                match result {
                    Ok(json) => {
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            // Client disconnected or send failed.
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        // Broadcast channel shut down (server stopping).
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        // The client is too slow; skip dropped packets but stay connected.
                        tracing::warn!("WebSocket client lagged — dropped {n} CSI packets");
                    }
                }
            }

            // ── Detect client-initiated close or disconnect ────────────────
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {} // Ignore pings / pong / unexpected binary frames.
                }
            }
        }
    }

    tracing::debug!("WebSocket client disconnected");
}
