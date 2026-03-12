use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc, watch};

use crate::models::DeviceConfig;

/// Shared application state, cheaply cloned into every route handler via Axum's `State` extractor.
#[derive(Clone)]
pub struct AppState {
    /// Send CLI command strings to the serial background task.
    pub cmd_tx: mpsc::Sender<String>,
    /// Broadcast raw CSI frame bytes to all connected WebSocket clients.
    pub csi_tx: broadcast::Sender<Vec<u8>>,
    /// Notify the serial task of log-mode changes (affects the frame delimiter).
    pub log_mode_tx: Arc<watch::Sender<String>>,
    /// Cached view of the current device configuration.
    pub config: Arc<Mutex<DeviceConfig>>,
}
