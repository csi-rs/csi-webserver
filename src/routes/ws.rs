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

/// Upgrade an HTTP connection to a WebSocket and stream raw CSI frames.
///
/// Each binary message sent to the client is one unmodified frame as received
/// from the ESP32 over serial. The client is responsible for decoding based
/// on the active log mode (e.g. array-list text or COBS binary).
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    let rx = state.csi_tx.subscribe();
    ws.on_upgrade(|socket| handle_socket(socket, rx))
}

async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<Vec<u8>>) {
    loop {
        tokio::select! {
            // ── Forward raw CSI frame to the WebSocket client ─────────────
            result = rx.recv() => {
                match result {
                    Ok(data) => {
                        if socket.send(Message::Binary(data.into())).await.is_err() {
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
